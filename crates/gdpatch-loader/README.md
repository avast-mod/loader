# `avast-loader`

Loads AVaSt into the target game process. Forked from GDPatch.

## Windows

Load into the process by placing next to the game executable as `winmm.dll` (rename the released `avast_loader.dll`, or let `install.ps1` do it).

On Wine, you need to set a DLL override. To do this in Steam, go to Properties > General > Launch Options and enter `WINEDLLOVERRIDES="winmm=n,b" %command%`.

## Linux

Load into the process by using the `LD_PRELOAD` environment variable (e.g. `LD_PRELOAD=/path/to/libavast_loader.so ./game`). `install.sh` prints the exact launch option for your Steam install.
