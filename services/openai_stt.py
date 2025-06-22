"""OpenAI Speech-to-Text Service Implementation"""
import os
import logging
from typing import Union, Optional
from pathlib import Path
from openai import OpenAI
import httpx

logger = logging.getLogger('voice_typing')


class OpenAITranscriber:
    """OpenAI STT service implementation supporting Whisper and GPT-4o models"""

    def __init__(self, model: str = "gpt-4o-mini-transcribe", language: str = "en"):
        """
        Initialize OpenAI transcriber

        Args:
            model: Model to use ('whisper-1', 'gpt-4o-transcribe', 'gpt-4o-mini-transcribe')
            language: Language code for transcription (e.g., 'en', 'es', 'fr')
        """
        api_key = os.environ.get("OPENAI_API_KEY")
        if not api_key:
            raise ValueError("OPENAI_API_KEY environment variable not set")

        self.client = OpenAI(
            api_key=api_key,
            # Configure timeout: 60s total timeout, 10s connect timeout
            timeout=httpx.Timeout(60.0, connect=10.0)
        )
        self.model = model
        self.language = language

    def transcribe(self, audio_data: Union[bytes, str, Path]) -> str:
        """
        Transcribe audio using OpenAI's API

        Args:
            audio_data: Either raw audio bytes, file path as string, or Path object

        Returns:
            Transcribed text

        Raises:
            Exception: If transcription fails
        """
        try:
            # Handle different input types
            if isinstance(audio_data, (str, Path)):
                # File path provided
                file_path = Path(audio_data) if isinstance(audio_data, str) else audio_data
                if not file_path.exists():
                    raise FileNotFoundError(f"Audio file not found: {file_path}")

                with open(file_path, 'rb') as audio_file:
                    response = self.client.audio.transcriptions.create(
                        model=self.model,
                        file=audio_file,
                        language=self.language
                    )
            else:
                # Raw bytes provided - need to create a file-like object
                # OpenAI API requires a file object with a name attribute
                import io
                audio_file = io.BytesIO(audio_data)
                audio_file.name = "audio.wav"  # OpenAI needs a filename

                response = self.client.audio.transcriptions.create(
                    model=self.model,
                    file=audio_file,
                    language=self.language
                )

            return response.text

        except Exception as e:
            logger.error(f"OpenAI transcription failed: {e}", exc_info=True)
            raise

    def update_model(self, model: str) -> None:
        """Update the model used for transcription"""
        # dead code? probably added with the intention of reusing transcriber instances
        # but the current implementation doesn't work that way?
        self.model = model

    def update_language(self, language: str) -> None:
        """Update the language used for transcription"""
        self.language = language
