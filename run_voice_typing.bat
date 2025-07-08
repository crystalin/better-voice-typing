@echo off
chcp 65001 >nul
title Voice Typing
cd "%~dp0"
start /b "" ".\.venv\Scripts\pythonw.exe" ".\voice_typing.pyw"
echo 🚀 Better Voice Typing is starting up...
echo    Please wait a few moments for the system tray icon to appear...
timeout /t 5 /nobreak >nul
exit