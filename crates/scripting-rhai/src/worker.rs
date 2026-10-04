//! One bounded worker. It owns only script roots/values/ASTs, never simulation references.
use super::*;
use rustcraft_control::JobRef;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
};

pub(crate) enum Task {
    Png {
        path: PathBuf,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    Compile {
        path: PathBuf,
        hash: Option<String>,
        command: bool,
    },
    Write {
        directory: PathBuf,
        files: Vec<(String, serde_json::Value)>,
        text: Vec<(String, String)>,
    },
}
pub(crate) enum Payload {
    Compiled(Option<Box<(LoadedScript, Option<rustcraft_control::CommandSpec>)>>),
    Written(PathBuf),
}
pub(crate) struct Request {
    pub id: JobRef,
    pub identity: String,
    pub generation: u64,
    pub task: Task,
}
pub(crate) struct Reply {
    pub id: JobRef,
    pub identity: String,
    pub generation: u64,
    pub result: ControlResult<Payload>,
}
pub(crate) struct Worker {
    tx: Option<mpsc::SyncSender<Request>>,
    rx: mpsc::Receiver<Reply>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Worker {
    pub fn new(root: ScriptRoot) -> Self {
        let (tx, requests) = mpsc::sync_channel::<Request>(16);
        let (results, rx) = mpsc::sync_channel(16);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = std::thread::Builder::new()
            .name("dx1-worker".into())
            .spawn(move || {
                let mut runtime = RhaiRuntime::new(
                    Context::read_only(rustcraft_control::Source::Script),
                    Limits::default(),
                );
                while let Ok(request) = requests.recv() {
                    if stopping.load(Ordering::Acquire) {
                        break;
                    }
                    let result = (|| match request.task {
                        Task::Png {
                            path,
                            width,
                            height,
                            rgba,
                        } => {
                            if let Some(parent) = path.parent() {
                                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                            }
                            let file =
                                std::fs::File::create_new(&path).map_err(|e| e.to_string())?;
                            let mut encoder = png::Encoder::new(file, width, height);
                            encoder.set_color(png::ColorType::Rgba);
                            encoder.set_depth(png::BitDepth::Eight);
                            encoder
                                .write_header()
                                .map_err(|e| e.to_string())?
                                .write_image_data(&rgba)
                                .map_err(|e| e.to_string())?;
                            Ok(Payload::Written(path))
                        }
                        Task::Compile {
                            path,
                            hash,
                            command,
                        } => {
                            let requested = path.clone();
                        let (path, source) = root.read(path)?;
                        if requested.components().collect::<Vec<_>>() != path.components().collect::<Vec<_>>() { return Err("script path aliases are not supported; use its canonical approved path".into()); }
                            let current = blake3::hash(source.as_bytes()).to_hex().to_string();
                            if hash.as_ref() == Some(&current) {
                                return Ok(Payload::Compiled(None));
                            }
                            let started = Instant::now();
                            let ast = runtime
                                .compile(&source)
                                .map_err(|e| format!("{}: {e}", path.display()))?;
                            let spec = if command {
                                Some(command_spec(&mut runtime, &ast)?)
                            } else {
                                None
                            };
                            Ok(Payload::Compiled(Some(Box::new((
                                LoadedScript {
                                    domains: rustcraft_control::diagnostics::query_domains(&source),
                                    path,
                                    ast,
                                    hash: current,
                                    generation: request.generation,
                                    last_error: None,
                                    compile_us: started.elapsed().as_micros(),
                                },
                                spec,
                            )))))
                        }
                        Task::Write {
                            directory,
                            files,
                            text,
                        } => {
                            std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
                            for (name, value) in files {
                                write_json(&directory.join(name), &value)?;
                            }
                            for (name, value) in text {
                                std::fs::write(directory.join(name), value)
                                    .map_err(|e| e.to_string())?;
                            }
                            Ok(Payload::Written(directory))
                        }
                    })();
                    // Bounded backpressure; shutdown never waits on a full publication queue.
                    let mut reply = Reply {
                        id: request.id,
                        identity: request.identity,
                        generation: request.generation,
                        result,
                    };
                    loop {
                        match results.try_send(reply) {
                            Ok(()) => break,
                            Err(mpsc::TrySendError::Full(value)) => {
                                if stopping.load(Ordering::Acquire) {
                                    return;
                                }
                                reply = value;
                                std::thread::park_timeout(Duration::from_millis(1));
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => return,
                        }
                    }
                }
            })
            .expect("create DX worker");
        Self {
            tx: Some(tx),
            rx,
            stop,
            thread: Some(thread),
        }
    }
    pub fn request(&self, request: Request) -> ControlResult<()> {
        self.tx
            .as_ref()
            .unwrap()
            .try_send(request)
            .map_err(|e| format!("DX worker queue: {e}"))
    }
    pub fn poll(&self) -> Option<Reply> {
        self.rx.try_recv().ok()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.tx.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
