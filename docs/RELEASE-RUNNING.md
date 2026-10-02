# Running RustCraft binaries

The client and server currently embed the `minecraft-b173` game package. The server headless smoke
check needs no game assets:

```sh
./rustcraft-server --smoke
```

The client requires a compatible, user-supplied Beta terrain sheet; it is intentionally absent from
the archive. Point the client at a legally obtained local file, then launch a new or named survival
world:

```sh
RUSTCRAFT_TERRAIN_TEXTURE=/path/to/terrain.png ./rustcraft-client --survival --world default
```

On Windows PowerShell:

```powershell
$env:RUSTCRAFT_TERRAIN_TEXTURE = "C:\path\to\terrain.png"
.\rustcraft-client.exe --survival --world default
```

World saves default to `./saves/` and can be redirected with `RUSTCRAFT_SAVES_DIR`; archives do
not contain or relocate saves. `--version` prints build identity without opening graphics or a
world.
