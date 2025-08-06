"""OpenAPI Speech-to-Text Service Implementation"""
import os
import logging
from typing import Union, Optional
from pathlib import Path
import io
import requests
import json

logger = logging.getLogger('voice_typing')


class OpenAPITranscriber:
    """OpenAPI STT service implementation for custom API endpoints"""

    def __init__(
        self,
        base_url: str = "http://localhost:8000",
        model: str = "parakeet-tdt-0.6b-v2",
        language: str = "en"
    ):
        """
        Initialize OpenAPI transcriber

        Args:
            base_url: Base URL of the OpenAPI endpoint
            model: Model to use for transcription
            language: Language code for transcription
        """
        self.base_url = base_url.rstrip('/')
        self.model = model
        self.language = language
        
        # Get API key if configured (optional)
        self.api_key = os.environ.get("OPENAPI_STT_API_KEY")
        
        logger.info(f"Initialized OpenAPI transcriber with URL: {self.base_url}, model: {model}")

    def transcribe(self, audio_data: Union[bytes, str, Path]) -> str:
        """
        Transcribe audio using OpenAPI endpoint

        Args:
            audio_data: Either raw audio bytes, file path as string, or Path object

        Returns:
            Transcribed text

        Raises:
            Exception: If transcription fails
        """
        try:
            # Prepare audio data
            if isinstance(audio_data, (str, Path)):
                file_path = Path(audio_data)
                if not file_path.exists():
                    raise FileNotFoundError(f"Audio file not found: {file_path}")
                with open(file_path, 'rb') as f:
                    audio_bytes = f.read()
                filename = file_path.name
            else:
                audio_bytes = audio_data
                filename = "audio.wav"

            # Add authorization header if API key is configured
            headers = {}
            if self.api_key:
                headers['Authorization'] = f"Bearer {self.api_key}"
            
            # The error shows the API expects 'file' field
            endpoint = f"{self.base_url}/transcribe"
            
            # Based on the error message, the API expects a 'file' field
            files = {
                'file': (filename, io.BytesIO(audio_bytes), 'audio/wav')
            }
            
            logger.debug(f"Sending request to {endpoint}")
            
            # First try: just the file (what the error indicates is required)
            try:
                response = requests.post(
                    endpoint,
                    files=files,
                    headers=headers,
                    timeout=60
                )
                
                if response.status_code == 200:
                    result = response.json()
                    
                    # Handle different response formats
                    if isinstance(result, dict):
                        # Check for segments format (like your API returns)
                        if 'segments' in result and isinstance(result['segments'], list):
                            # Extract text from all segments and join them
                            texts = []
                            for segment in result['segments']:
                                if isinstance(segment, dict) and 'text' in segment:
                                    texts.append(segment['text'])
                            if texts:
                                return ' '.join(texts)
                        
                        # Try common field names
                        for field in ['text', 'transcription', 'result', 'transcript', 'output', 'response', 'data']:
                            if field in result:
                                return str(result[field])
                        
                        # If no known field, return the whole dict as string
                        logger.warning(f"Unknown response format: {result}")
                        return str(result)
                    elif isinstance(result, str):
                        return result
                    else:
                        return str(result)
                        
                elif response.status_code == 422:
                    # If it still fails with 422, try adding model parameter
                    logger.debug(f"Got 422, trying with model parameter")
                    files = {
                        'file': (filename, io.BytesIO(audio_bytes), 'audio/wav')
                    }
                    data = {
                        'model': self.model
                    }
                    
                    response = requests.post(
                        endpoint,
                        files=files,
                        data=data,
                        headers=headers,
                        timeout=60
                    )
                    
                    if response.status_code == 200:
                        result = response.json()
                        
                        # Handle different response formats
                        if isinstance(result, dict):
                            # Check for segments format (like your API returns)
                            if 'segments' in result and isinstance(result['segments'], list):
                                # Extract text from all segments and join them
                                texts = []
                                for segment in result['segments']:
                                    if isinstance(segment, dict) and 'text' in segment:
                                        texts.append(segment['text'])
                                if texts:
                                    return ' '.join(texts)
                            
                            # Try common field names
                            for field in ['text', 'transcription', 'result', 'transcript', 'output', 'response', 'data']:
                                if field in result:
                                    return str(result[field])
                            return str(result)
                        elif isinstance(result, str):
                            return result
                        else:
                            return str(result)
                    else:
                        error_msg = f"OpenAPI transcription failed. HTTP {response.status_code}: {response.text}"
                        logger.error(error_msg)
                        raise RuntimeError(error_msg)
                else:
                    error_msg = f"OpenAPI transcription failed. HTTP {response.status_code}: {response.text}"
                    logger.error(error_msg)
                    raise RuntimeError(error_msg)
                    
            except requests.exceptions.ConnectionError as e:
                error_msg = f"Connection failed to {endpoint}: {e}"
                logger.error(error_msg)
                raise RuntimeError(error_msg)
            except requests.exceptions.Timeout as e:
                error_msg = f"Request timeout to {endpoint}: {e}"
                logger.error(error_msg)
                raise RuntimeError(error_msg)
                
        except Exception as e:
            logger.error(f"OpenAPI transcription failed: {e}", exc_info=True)
            raise

    def update_model(self, model: str) -> None:
        """Update the model used for transcription"""
        self.model = model
        logger.info(f"Updated OpenAPI STT model to: {model}")

    def update_language(self, language: str) -> None:
        """Update the language used for transcription"""
        self.language = language
        logger.info(f"Updated OpenAPI STT language to: {language}")
    
    def update_base_url(self, base_url: str) -> None:
        """Update the base URL for the OpenAPI endpoint"""
        self.base_url = base_url.rstrip('/')
        logger.info(f"Updated OpenAPI STT base URL to: {self.base_url}")