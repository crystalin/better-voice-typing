# Speech-to-Text Provider Migration Guide

## What's New in v0.6.0

We've upgraded the Voice Typing Assistant to support OpenAI's newest GPT-4o Transcribe models and (begin to) support multiple speech-to-text providers such as Google Cloud, which means:

- **Better accuracy** - GPT-4o models have lower word error rates than Whisper
- **Lower cost** - GPT-4o Mini is ~50% cheaper than Whisper
- **Future-proof architecture** - Easy to add new providers like Google Cloud, Azure, etc.

## Default Changes

- **New default model**: GPT-4o Transcribe (was Whisper-1)
- **Extensible provider system**: Switch between providers from the tray menu

## How to Use

### Selecting a Speech-to-Text Provider

1. Right-click the tray icon
2. Go to Settings → Speech-to-Text → Provider
3. Choose your provider (currently OpenAI, with Google Cloud coming "soon")

### Selecting an OpenAI Model

Settings → Speech-to-Text → OpenAI Model, then choose from:
- **GPT-4o (Best)** - Best accuracy, ideal for professional use
- **GPT-4o Mini** - Great balance of speed, cost, and quality (default)
- **Whisper (Legacy)** - The classic model, good baseline accuracy

### Model Comparison

| Model | Relative Cost | Speed | Accuracy | Best For |
|-------|--------------|-------|----------|----------|
| GPT-4o | $0.006/min | Fast | Best | Professional transcription (recommended) |
| GPT-4o Mini | $0.003/min | Fast | Better | Budget-friendly use |
| Whisper-1 | $0.006/min | Slow | Good | Legacy compatibility |

## Backward Compatibility

- Run `setup.bat` to update, install the new dependencies, and migrate your settings
- Your existing settings are preserved
- The app will continue to work with your current OpenAI API key
- You can switch back to Whisper-1 at any time from the tray menu

## Future Providers

The new architecture makes it easy to add support for:
- Google Cloud Speech-to-Text
- Azure Cognitive Services
- Amazon Transcribe
- Local models? (Whisper running on your machine)