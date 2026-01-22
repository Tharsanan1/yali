# Protocol Adapters

Yali Gateway supports multiple AI providers through protocol adapters that automatically transform requests and responses between different formats.

## Supported Providers

| Provider | Type ID | Endpoint Pattern | Default Auth |
|----------|---------|------------------|--------------|
| OpenAI | `openai` | `api.openai.com` | Bearer token |
| Azure OpenAI | `azure_openai` | `*.openai.azure.com` | `api-key` header |
| Anthropic | `anthropic` | `api.anthropic.com` | `x-api-key` header |
| Google AI | `google_ai` | `generativelanguage.googleapis.com` | API key in URL |
| AWS Bedrock | `bedrock` | `bedrock-runtime.*.amazonaws.com` | AWS SigV4 |
| Custom | `custom` | Any URL | Configurable |

## How Adapters Work

The gateway uses OpenAI-compatible format as the canonical internal format. When you send a request, the adapter:

1. **Request Transformation**: Converts your OpenAI-format request to the provider's format
2. **Authentication**: Injects the appropriate authentication headers
3. **URL Rewriting**: Adjusts the path and query parameters for the provider
4. **Response Transformation**: Converts the provider's response back to OpenAI format

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         REQUEST TRANSFORMATION                               │
├─────────────────────────────────────────────────────────────────────────────┤
│  Incoming       ┌──────────┐   ┌──────────┐   ┌──────────┐   Outgoing      │
│  Request   ───► │   Auth   │──►│  Headers │──►│   URL    │──►  Request     │
│  (Client)       │ Injection│   │ Transform│   │ Rewrite  │   (to Provider) │
│                 └──────────┘   └──────────┘   └──────────┘                 │
│                                      │                                      │
│                                      ▼                                      │
│                              ┌──────────────┐                               │
│                              │     Body     │                               │
│                              │ Transformation│                              │
│                              │  (Request)   │                               │
│                              └──────────────┘                               │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Provider Configuration Examples

### OpenAI

The simplest configuration since OpenAI format is the canonical format:

```json
{
  "id": "provider_openai",
  "name": "OpenAI Production",
  "spec": {
    "type": "openai",
    "endpoint": "https://api.openai.com",
    "adapter": {
      "auth": {
        "type": "bearer",
        "secret_ref": "env://OPENAI_API_KEY"
      },
      "url": {
        "path_prefix": "/v1/chat/completions"
      },
      "headers": {
        "add": {"OpenAI-Organization": "org-123"}
      }
    }
  }
}
```

### Azure OpenAI

Azure OpenAI uses deployment names in the URL instead of model names:

```json
{
  "id": "provider_azure",
  "name": "Azure OpenAI",
  "spec": {
    "type": "azure_openai",
    "endpoint": "https://myresource.openai.azure.com",
    "adapter": {
      "auth": {
        "type": "header",
        "key": "api-key",
        "secret_ref": "env://AZURE_OPENAI_KEY"
      },
      "url": {
        "query_params": {
          "api-version": "2024-02-15-preview"
        }
      }
    }
  }
}
```

The adapter automatically:
- Removes the `model` field from the request body
- Constructs the path `/openai/deployments/{model}/chat/completions`
- Adds the `api-version` query parameter

### Anthropic (Claude)

Anthropic has a different message format:

```json
{
  "id": "provider_anthropic",
  "name": "Anthropic Claude",
  "spec": {
    "type": "anthropic",
    "endpoint": "https://api.anthropic.com",
    "adapter": {
      "auth": {
        "type": "header",
        "key": "x-api-key",
        "secret_ref": "env://ANTHROPIC_API_KEY"
      },
      "headers": {
        "add": {"anthropic-version": "2023-06-01"}
      }
    }
  }
}
```

The adapter automatically:
- Extracts system messages to a top-level `system` field
- Renames `stop` to `stop_sequences`
- Transforms the response format back to OpenAI

### Google AI (Gemini)

Google AI uses a different structure for messages:

```json
{
  "id": "provider_google",
  "name": "Google Gemini",
  "spec": {
    "type": "google_ai",
    "endpoint": "https://generativelanguage.googleapis.com",
    "adapter": {
      "auth": {
        "type": "query_param",
        "key": "key",
        "secret_ref": "env://GOOGLE_AI_KEY"
      }
    }
  }
}
```

The adapter automatically:
- Transforms `messages` to `contents` with `parts` array
- Maps role `assistant` to `model`
- Wraps `temperature`, `max_tokens`, etc. in `generationConfig`
- Handles `systemInstruction` separately

### AWS Bedrock

For Claude models on AWS Bedrock:

```json
{
  "id": "provider_bedrock",
  "name": "AWS Bedrock Claude",
  "spec": {
    "type": "bedrock",
    "endpoint": "https://bedrock-runtime.us-east-1.amazonaws.com",
    "adapter": {
      "auth": {
        "type": "aws_sigv4",
        "secret_ref": "env://AWS_CREDENTIALS"
      }
    }
  }
}
```

The adapter automatically:
- Adds `anthropic_version` to the request
- Extracts system messages
- Transforms stop sequences

## Authentication Types

| Type | Description | Configuration |
|------|-------------|---------------|
| `bearer` | Bearer token in Authorization header | `{"type": "bearer", "secret_ref": "env://API_KEY"}` |
| `header` | Custom header with API key | `{"type": "header", "key": "x-api-key", "secret_ref": "..."}` |
| `query_param` | API key in URL query parameter | `{"type": "query_param", "key": "key", "secret_ref": "..."}` |
| `none` | No authentication | `{"type": "none"}` |

## Secret References

Secrets can be referenced using the following formats:

| Format | Description | Example |
|--------|-------------|---------|
| `env://VAR_NAME` | Environment variable | `env://OPENAI_API_KEY` |

## Field Mappings

The following table shows how fields are mapped between providers:

| Canonical (OpenAI) | Anthropic | Google AI | Azure OpenAI | Bedrock |
|--------------------|-----------|-----------|--------------|---------|
| `model` | `model` | URL path | URL path | URL path |
| `messages` | `messages` | `contents` | `messages` | `messages` |
| `messages[].role` | `role` | `role` | `role` | `role` |
| `messages[].content` | `content` | `parts[].text` | `content` | `content` |
| `temperature` | `temperature` | `generationConfig.temperature` | `temperature` | `temperature` |
| `max_tokens` | `max_tokens` | `generationConfig.maxOutputTokens` | `max_tokens` | `max_tokens` |
| `stop` | `stop_sequences` | `generationConfig.stopSequences` | `stop` | `stop_sequences` |

## Custom Defaults

You can set default values that are injected if not present in the request:

```json
{
  "adapter": {
    "request_body": {
      "defaults": {
        "temperature": 0.7,
        "max_tokens": 1024
      }
    }
  }
}
```

## Removing Fields

You can remove fields before sending to the provider:

```json
{
  "adapter": {
    "request_body": {
      "remove_fields": ["user", "stream"]
    }
  }
}
```

## Multi-Provider Failover

Combine adapters with load balancing for seamless failover:

```json
{
  "backends": [{
    "id": "backend_multi",
    "spec": {
      "load_balancing": {"algorithm": "failover"},
      "providers": [
        {"ref": "provider_openai", "priority": 1},
        {"ref": "provider_azure", "priority": 2},
        {"ref": "provider_anthropic", "priority": 3}
      ]
    }
  }]
}
```

If OpenAI fails, the gateway automatically tries Azure, then Anthropic—all transparently to the client!
