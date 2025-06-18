import os
from dotenv import load_dotenv
from openai import OpenAI
import httpx

# OpenAI Speech to text docs: https://platform.openai.com/docs/guides/speech-to-text
# ⚠️ IMPORTANT: OpenAI Audio API file uploads are currently limited to 25 MB

load_dotenv()

client = OpenAI(
    api_key=os.environ.get("OPENAI_API_KEY"),
    # Configure timeout: 60s total timeout, 10s connect timeout
    timeout=httpx.Timeout(60.0, connect=10.0)
)

def transcribe_audio(filename: str, language: str = "en") -> str:
    with open(filename, 'rb') as audio_file:
        response = client.audio.transcriptions.create(
            model="whisper-1",
            file=audio_file,
            language=language
        )
    return response.text