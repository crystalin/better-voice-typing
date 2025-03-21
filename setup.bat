@echo off
setlocal EnableDelayedExpansion

echo Voice Typing Assistant Setup/Update Tool

REM Check if Python is installed
python --version > nul 2>&1
if errorlevel 1 (
    echo Python is not installed! Please install Python 3.8 or newer from python.org
    pause
    exit /b 1
)

REM Verify Python version is 3.8+
for /f "tokens=2 delims=." %%I in ('python -c "import sys; print(sys.version.split()[0])"') do set PYVER=%%I
if "%PYVER%" LSS "8" (
    echo Error: Python 3.8 or newer is required
    pause
    exit /b 1
)

REM Check if uv is installed
uv --version > nul 2>&1
if errorlevel 1 (
    echo Error: uv is not installed
    echo Please install uv from https://docs.astral.sh/uv/getting-started/#installation
    echo You can run: curl -sSf https://astral.sh/uv/install.ps1 ^| powershell
    pause
    exit /b 1
)

REM Check if this is an update or first install
if exist .venv (
    echo Existing installation detected
    choice /C YN /M "Would you like to check for updates"
    if errorlevel 2 goto :SKIP_UPDATE

    echo Checking for updates...
    call .venv\Scripts\activate.bat
    python check_update.py
    if errorlevel 1 (
        echo Update failed. Please try again later.
    ) else (
        echo Updating dependencies...
        uv pip install -r requirements.txt
    )
    goto :END
)

:SKIP_UPDATE
REM First time setup continues here...
echo Creating virtual environment with uv...
uv venv --python ">=3.8"

REM Activate virtual environment and install requirements
echo Installing required packages with uv...
call .venv\Scripts\activate
uv pip install -r requirements.txt

REM Create .env file if it doesn't exist
if not exist .env (
    echo Creating configuration file...
    echo OPENAI_API_KEY=> .env
    echo ANTHROPIC_API_KEY=> .env

    echo Please enter your OpenAI API key:
    set /p OPENAI_KEY="> "
    echo OPENAI_API_KEY=%OPENAI_KEY%> .env

    echo.
    echo Optional: Enter your Anthropic API key for text cleaning (or press Enter to skip):
    set /p ANTHROPIC_KEY="> "
    if not "%ANTHROPIC_KEY%"=="" (
        echo ANTHROPIC_API_KEY=%ANTHROPIC_KEY%>> .env
    )
)

:END
echo.
echo Setup/Update complete! You can now run voice_typing.pyw to start the app.
echo.
choice /C YN /M "Would you like to launch the application now"
if errorlevel 2 goto :EXIT
echo Launching Voice Typing Assistant...
start pythonw voice_typing.pyw
goto :EXIT

:EXIT
echo Exiting setup...
pause