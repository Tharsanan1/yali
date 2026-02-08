Feature: Route policy config merge semantics

  Scenario: Route params deep-merge nested objects with policy defaults
    Given the control plane is running
    When I POST "/policies" on the control plane with JSON:
      """
      {
        "id": "authn-merge-nested",
        "version": "1.0.0",
        "wasm_uri": "{{policy_add_header_wasm_uri}}",
        "sha256": "{{policy_add_header_sha256}}",
        "supported_stages": ["pre_route"],
        "config_schema": {
          "type": "object",
          "required": ["auth"],
          "properties": {
            "auth": {
              "type": "object",
              "required": ["issuer", "required_scopes"],
              "properties": {
                "issuer": { "type": "string" },
                "required_scopes": {
                  "type": "array",
                  "items": { "type": "string" },
                  "minItems": 1
                }
              },
              "additionalProperties": false
            }
          },
          "additionalProperties": false
        },
        "default_config": {
          "auth": {
            "issuer": "https://issuer.example",
            "required_scopes": ["read:users"]
          }
        }
      }
      """
    Then the response status should be 201
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "route-merge-nested",
        "match": { "path_prefix": "/v1/merge-nested", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": [
          {
            "stage": "pre_route",
            "id": "authn-merge-nested",
            "version": "1.0.0",
            "params": {
              "auth": {
                "required_scopes": ["read:users", "list:users"]
              }
            }
          }
        ]
      }
      """
    Then the response status should be 201

  Scenario: Route params replace arrays instead of appending
    Given the control plane is running
    When I POST "/policies" on the control plane with JSON:
      """
      {
        "id": "authn-merge-array",
        "version": "1.0.0",
        "wasm_uri": "{{policy_add_header_wasm_uri}}",
        "sha256": "{{policy_add_header_sha256}}",
        "supported_stages": ["pre_route"],
        "config_schema": {
          "type": "object",
          "required": ["scopes"],
          "properties": {
            "scopes": {
              "type": "array",
              "items": { "type": "string" },
              "maxItems": 1,
              "minItems": 1
            }
          },
          "additionalProperties": false
        },
        "default_config": {
          "scopes": ["read:users"]
        }
      }
      """
    Then the response status should be 201
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "route-merge-array",
        "match": { "path_prefix": "/v1/merge-array", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": [
          {
            "stage": "pre_route",
            "id": "authn-merge-array",
            "version": "1.0.0",
            "params": {
              "scopes": ["write:users"]
            }
          }
        ]
      }
      """
    Then the response status should be 201
