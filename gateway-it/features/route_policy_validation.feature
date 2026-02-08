Feature: Route policy validation

  Scenario: Reject route when referenced policy does not exist
    Given the control plane is running
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "route-policy-missing",
        "match": { "path_prefix": "/v1/policy-missing", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": [
          {
            "stage": "pre_route",
            "id": "missing-policy",
            "version": "9.9.9",
            "params": {}
          }
        ]
      }
      """
    Then the response status should be 422
    And the JSON response should include:
      """
      { "error": "validation_error" }
      """

  Scenario: Reject route when stage is not supported by policy
    Given the control plane is running
    When I POST "/policies" on the control plane with JSON:
      """
      {
        "id": "authn-stage-check",
        "version": "1.0.0",
        "wasm_uri": "file:///policies/authn.wasm",
        "sha256": "deadbeef",
        "supported_stages": ["pre_route"],
        "config_schema": { "type": "object" },
        "default_config": {}
      }
      """
    Then the response status should be 201
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "route-stage-invalid",
        "match": { "path_prefix": "/v1/stage-invalid", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": [
          {
            "stage": "post_response",
            "id": "authn-stage-check",
            "version": "1.0.0",
            "params": {}
          }
        ]
      }
      """
    Then the response status should be 422
    And the JSON response should include:
      """
      { "error": "validation_error" }
      """

  Scenario: Reject route params with plaintext secret
    Given the control plane is running
    When I POST "/policies" on the control plane with JSON:
      """
      {
        "id": "authn-secret-policy",
        "version": "1.0.0",
        "wasm_uri": "file:///policies/authn.wasm",
        "sha256": "deadbeef",
        "supported_stages": ["pre_route"],
        "config_schema": {
          "type": "object",
          "required": ["jwks"],
          "properties": {
            "jwks": {
              "type": "object",
              "required": ["$secret"],
              "properties": {
                "$secret": { "type": "string", "pattern": "^vault://" }
              },
              "additionalProperties": false
            }
          },
          "additionalProperties": false
        },
        "default_config": {
          "jwks": { "$secret": "vault://prod/auth/jwks_url" }
        }
      }
      """
    Then the response status should be 201
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "route-secret-plaintext",
        "match": { "path_prefix": "/v1/secret-plaintext", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": [
          {
            "stage": "pre_route",
            "id": "authn-secret-policy",
            "version": "1.0.0",
            "params": {
              "jwks": "https://issuer.example/.well-known/jwks.json"
            }
          }
        ]
      }
      """
    Then the response status should be 422
    And the JSON response should include:
      """
      { "error": "validation_error" }
      """

  Scenario: Accept route params with secret reference object
    Given the control plane is running
    When I POST "/policies" on the control plane with JSON:
      """
      {
        "id": "authn-secret-policy-valid",
        "version": "1.0.0",
        "wasm_uri": "file:///policies/authn.wasm",
        "sha256": "deadbeef",
        "supported_stages": ["pre_route"],
        "config_schema": {
          "type": "object",
          "required": ["jwks"],
          "properties": {
            "jwks": {
              "type": "object",
              "required": ["$secret"],
              "properties": {
                "$secret": { "type": "string", "pattern": "^vault://" }
              },
              "additionalProperties": false
            }
          },
          "additionalProperties": false
        },
        "default_config": {
          "jwks": { "$secret": "vault://prod/auth/jwks_url" }
        }
      }
      """
    Then the response status should be 201
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "route-secret-ref",
        "match": { "path_prefix": "/v1/secret-ref", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": [
          {
            "stage": "pre_route",
            "id": "authn-secret-policy-valid",
            "version": "1.0.0",
            "params": {
              "jwks": { "$secret": "vault://prod/auth/jwks_url" }
            }
          }
        ]
      }
      """
    Then the response status should be 201

  Scenario: Reject invalid route update and keep prior route config
    Given the control plane is running
    When I POST "/policies" on the control plane with JSON:
      """
      {
        "id": "authn-update-policy",
        "version": "1.0.0",
        "wasm_uri": "file:///policies/authn.wasm",
        "sha256": "deadbeef",
        "supported_stages": ["pre_route"],
        "config_schema": {
          "type": "object",
          "required": ["jwks", "required_scopes"],
          "properties": {
            "jwks": {
              "type": "object",
              "required": ["$secret"],
              "properties": {
                "$secret": { "type": "string", "pattern": "^vault://" }
              },
              "additionalProperties": false
            },
            "required_scopes": {
              "type": "array",
              "items": { "type": "string" },
              "minItems": 1
            }
          },
          "additionalProperties": false
        },
        "default_config": {
          "jwks": { "$secret": "vault://prod/auth/jwks_url" },
          "required_scopes": ["read:users"]
        }
      }
      """
    Then the response status should be 201
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "route-update-guard",
        "match": { "path_prefix": "/v1/update-guard", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": [
          {
            "stage": "pre_route",
            "id": "authn-update-policy",
            "version": "1.0.0",
            "params": {
              "required_scopes": ["read:users"]
            }
          }
        ]
      }
      """
    Then the response status should be 201
    When I PUT "/routes/route-update-guard" on the control plane with JSON:
      """
      {
        "id": "route-update-guard",
        "match": { "path_prefix": "/v1/update-guard", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": [
          {
            "stage": "pre_route",
            "id": "authn-update-policy",
            "version": "1.0.0",
            "params": {
              "jwks": "plaintext-secret-value"
            }
          }
        ]
      }
      """
    Then the response status should be 422
    And the JSON response should include:
      """
      { "error": "validation_error" }
      """
    When I GET "/routes/route-update-guard" on the control plane
    Then the response status should be 200
    And the JSON response should include:
      """
      { "policies": [{ "params": { "required_scopes": ["read:users"] } }] }
      """
