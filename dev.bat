@echo off
:: Launches the Tauri dev server from within the VS Developer environment.
:: Run this instead of "npm run tauri dev" directly.
call "C:\Program Files\Microsoft Visual Studio\18\Community\Common7\Tools\VsDevCmd.bat" -arch=amd64 >nul 2>&1
cd /d "%~dp0"
npm run tauri dev
