Feature: Policy registration validation

  Scenario: Register policy with valid schema and defaults
    Given the control plane is running
    When I POST "/policies" on the control plane with JSON:
      """
      {
        "id": "authn-schema-valid",
        "version": "1.0.0",
        "wasm_uri": "{{policy_add_header_wasm_uri}}",
        "sha256": "{{policy_add_header_sha256}}",
        "supported_stages": ["pre_route"],
        "config_schema": {
          "type": "object",
          "required": ["required_scopes"],
          "properties": {
            "required_scopes": {
              "type": "array",
              "items": { "type": "string" },
              "minItems": 1
            }
          },
          "additionalProperties": false
        },
        "default_config": {
          "required_scopes": ["read:users"]
        }
      }
      """
    Then the response status should be 201

  Scenario: Reject policy with invalid JSON schema
    Given the control plane is running
    When I POST "/policies" on the control plane with JSON:
      """
      {
        "id": "authn-schema-invalid",
        "version": "1.0.0",
        "wasm_uri": "{{policy_add_header_wasm_uri}}",
        "sha256": "{{policy_add_header_sha256}}",
        "config_schema": {
          "type": 42
        },
        "default_config": {}
      }
      """
    Then the response status should be 422
    And the JSON response should include:
      """
      { "error": "validation_error" }
      """

  Scenario: Reject policy when defaults do not match schema
    Given the control plane is running
    When I POST "/policies" on the control plane with JSON:
      """
      {
        "id": "authn-defaults-invalid",
        "version": "1.0.0",
        "wasm_uri": "{{policy_add_header_wasm_uri}}",
        "sha256": "{{policy_add_header_sha256}}",
        "config_schema": {
          "type": "object",
          "required": ["required_scopes"],
          "properties": {
            "required_scopes": {
              "type": "array",
              "items": { "type": "string" },
              "minItems": 1
            }
          },
          "additionalProperties": false
        },
        "default_config": {}
      }
      """
    Then the response status should be 422
    And the JSON response should include:
      """
      { "error": "validation_error" }
      """
