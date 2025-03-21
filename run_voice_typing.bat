@echo off
chcp 65001 >nul
title Voice Typing
cd "%~dp0"
start /b "" ".\.venv\Scripts\pythonw.exe" ".\voice_typing.pyw"
echo 🚀 Better Voice Typing is launching...
timeout /t 3 /nobreak >nul
exit