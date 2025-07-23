"""OpenAI Speech-to-Text Service Implementation"""
import os
import logging
from typing import Union, Optional
from pathlib import Path
import io
import soundfile as sf
import numpy as np
from openai import OpenAI
import httpx

logger = logging.getLogger('voice_typing')

# NOTE: Temp workaround for OpenAI bug where transcription cuts off.
# See: https://community.openai.com/t/gpt-4o-transcribe-truncates-the-transcript/1148347
PADDING_DURATION_S = 1.5
NOISE_AMPLITUDE = 0.08


def _pad_audio_with_noise(
    audio_data: Union[bytes, str, Path],
    duration_s: float,
    amplitude: float
) -> io.BytesIO:
    """
    Pads audio data with quiet brown noise at the end for a more organic sound.

    Args:
        audio_data: Audio data as bytes, file path string, or Path object.
        duration_s: Duration of noise to add in seconds.
        amplitude: The peak amplitude of the noise.

    Returns:
        An in-memory BytesIO object containing the padded audio in WAV format.
    """
    input_stream = io.BytesIO(audio_data) if isinstance(audio_data, bytes) else audio_data
    data, samplerate = sf.read(input_stream, dtype='float32')

    padding_samples = int(duration_s * samplerate)

    # Generate white noise and integrate it to create brown noise (more "natural" sounding)
    white_noise = np.random.randn(padding_samples).astype('float32')
    brown_noise_unscaled = np.cumsum(white_noise)

    # Normalize to the target amplitude to prevent clipping
    max_abs_val = np.max(np.abs(brown_noise_unscaled))
    if max_abs_val > 0:
        noise = (brown_noise_unscaled / max_abs_val) * amplitude
    else:
        noise = np.zeros_like(brown_noise_unscaled)

    padded_data = np.concatenate([data, noise])

    buffer = io.BytesIO()
    sf.write(buffer, padded_data, samplerate, format='WAV', subtype='PCM_16')
    buffer.seek(0)
    return buffer


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
            file_to_send = None
            opened_file = None

            try:
                # Conditionally pad audio for gpt-4o models as a workaround
                if "gpt-4o" in self.model:
                    logger.debug(f"Padding audio with {PADDING_DURATION_S}s of quiet noise for {self.model}")
                    padded_buffer = _pad_audio_with_noise(
                        audio_data, PADDING_DURATION_S, NOISE_AMPLITUDE
                    )

                    filename = "audio.wav"
                    if isinstance(audio_data, (str, Path)):
                        filename = Path(audio_data).name

                    padded_buffer.name = filename
                    file_to_send = padded_buffer

                # Handle original, unpadded audio data
                elif isinstance(audio_data, (str, Path)):
                    file_path = Path(audio_data)
                    if not file_path.exists():
                        raise FileNotFoundError(f"Audio file not found: {file_path}")
                    opened_file = open(file_path, 'rb')
                    file_to_send = opened_file
                else:
                    file_to_send = io.BytesIO(audio_data)
                    file_to_send.name = "audio.wav"

                # Perform transcription
                response = self.client.audio.transcriptions.create(
                    model=self.model,
                    file=file_to_send,
                    language=self.language
                )
                return response.text

            finally:
                if opened_file:
                    opened_file.close()
                # BytesIO buffers are managed by garbage collector, no close needed for file_to_send

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
