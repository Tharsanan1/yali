#!/usr/bin/env python3
"""
Mock AI API Server for testing the AI Gateway.

Simulates OpenAI-compatible endpoints with both regular and streaming responses.
Logs all request details for debugging.
"""

import json
import time
from datetime import datetime
from flask import Flask, request, Response, jsonify
import logging

app = Flask(__name__)

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


def log_request():
    """Log detailed request information."""
    logger.info("=" * 80)
    logger.info(f"📨 Incoming Request")
    logger.info(f"   Method: {request.method}")
    logger.info(f"   Path: {request.path}")
    logger.info(f"   Full URL: {request.url}")
    logger.info(f"   Remote Address: {request.remote_addr}")
    
    logger.info(f"   Headers:")
    for header, value in request.headers:
        logger.info(f"      {header}: {value}")
    
    if request.data:
        try:
            body = json.loads(request.data)
            logger.info(f"   Body: {json.dumps(body, indent=2)}")
        except:
            logger.info(f"   Body (raw): {request.data.decode('utf-8', errors='replace')}")
    
    logger.info("=" * 80)


def generate_chat_completion_stream(model, messages):
    """Generate SSE stream for chat completion."""
    # First chunk with role
    chunk1 = {
        "id": "chatcmpl-mock123",
        "object": "chat.completion.chunk",
        "created": int(time.time()),
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {"role": "assistant", "content": ""},
            "finish_reason": None
        }]
    }
    yield f"data: {json.dumps(chunk1)}\n\n"
    time.sleep(0.1)
    
    # Content chunks
    response_text = "Hello! I'm a mock AI assistant. This is a streaming response from the mock server. "
    response_text += "The gateway successfully forwarded your request! "
    response_text += f"You sent {len(messages)} message(s). "
    
    words = response_text.split()
    for word in words:
        chunk = {
            "id": "chatcmpl-mock123",
            "object": "chat.completion.chunk",
            "created": int(time.time()),
            "model": model,
            "choices": [{
                "index": 0,
                "delta": {"content": word + " "},
                "finish_reason": None
            }]
        }
        yield f"data: {json.dumps(chunk)}\n\n"
        time.sleep(0.05)  # Simulate typing delay
    
    # Final chunk
    chunk_final = {
        "id": "chatcmpl-mock123",
        "object": "chat.completion.chunk",
        "created": int(time.time()),
        "model": model,
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop"
        }]
    }
    yield f"data: {json.dumps(chunk_final)}\n\n"
    yield "data: [DONE]\n\n"


@app.route('/v1/chat/completions', methods=['POST'])
def chat_completions():
    """Handle chat completion requests (OpenAI-compatible)."""
    log_request()
    
    try:
        data = request.get_json()
        model = data.get('model', 'gpt-3.5-turbo')
        messages = data.get('messages', [])
        stream = data.get('stream', False)
        
        logger.info(f"🤖 Processing chat completion request")
        logger.info(f"   Model: {model}")
        logger.info(f"   Stream: {stream}")
        logger.info(f"   Messages: {len(messages)}")
        
        if stream:
            logger.info("📡 Starting streaming response...")
            return Response(
                generate_chat_completion_stream(model, messages),
                mimetype='text/event-stream',
                headers={
                    'Cache-Control': 'no-cache',
                    'X-Accel-Buffering': 'no',
                    'Connection': 'keep-alive'
                }
            )
        else:
            # Non-streaming response
            response = {
                "id": "chatcmpl-mock123",
                "object": "chat.completion",
                "created": int(time.time()),
                "model": model,
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": f"Hello! I'm a mock AI assistant. This is a non-streaming response from the mock server. The gateway successfully forwarded your request with {len(messages)} message(s)."
                    },
                    "finish_reason": "stop"
                }],
                "usage": {
                    "prompt_tokens": 20,
                    "completion_tokens": 30,
                    "total_tokens": 50
                }
            }
            
            logger.info(f"✅ Sending non-streaming response")
            return jsonify(response)
    
    except Exception as e:
        logger.error(f"❌ Error processing request: {e}")
        return jsonify({
            "error": {
                "message": str(e),
                "type": "mock_server_error",
                "code": "internal_error"
            }
        }), 500


@app.route('/v1/completions', methods=['POST'])
def completions():
    """Handle text completion requests."""
    log_request()
    
    try:
        data = request.get_json()
        model = data.get('model', 'text-davinci-003')
        prompt = data.get('prompt', '')
        stream = data.get('stream', False)
        
        logger.info(f"📝 Processing text completion request")
        logger.info(f"   Model: {model}")
        logger.info(f"   Stream: {stream}")
        
        response = {
            "id": "cmpl-mock123",
            "object": "text_completion",
            "created": int(time.time()),
            "model": model,
            "choices": [{
                "text": f"Mock completion for prompt: {prompt[:50]}...",
                "index": 0,
                "finish_reason": "stop"
            }]
        }
        
        logger.info(f"✅ Sending completion response")
        return jsonify(response)
    
    except Exception as e:
        logger.error(f"❌ Error processing request: {e}")
        return jsonify({"error": {"message": str(e)}}), 500


@app.route('/health', methods=['GET'])
def health():
    """Health check endpoint."""
    log_request()
    return jsonify({
        "status": "healthy",
        "service": "mock-ai-server",
        "timestamp": datetime.utcnow().isoformat()
    })


@app.route('/<path:path>', methods=['GET', 'POST', 'PUT', 'DELETE', 'PATCH'])
def catch_all(path):
    """Catch-all route for debugging."""
    log_request()
    logger.warning(f"⚠️  Unhandled path: /{path}")
    return jsonify({
        "error": {
            "message": f"Path /{path} is not implemented in mock server",
            "type": "not_found"
        }
    }), 404


if __name__ == '__main__':
    logger.info("🚀 Starting Mock AI Server")
    logger.info("   Port: 8000")
    logger.info("   Endpoints:")
    logger.info("      POST /v1/chat/completions - Chat completions (supports streaming)")
    logger.info("      POST /v1/completions - Text completions")
    logger.info("      GET  /health - Health check")
    logger.info("")
    logger.info("   Test with:")
    logger.info("      curl -X POST http://localhost:8000/v1/chat/completions \\")
    logger.info("        -H 'Content-Type: application/json' \\")
    logger.info("        -d '{\"model\":\"gpt-4\",\"messages\":[{\"role\":\"user\",\"content\":\"Hi\"}]}'")
    logger.info("")
    
    app.run(host='0.0.0.0', port=8000, debug=True, threaded=True)
