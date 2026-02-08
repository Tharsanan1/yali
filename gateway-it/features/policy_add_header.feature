Feature: WASM policy execution for add-header

  Scenario: Pre-upstream add-header appends and overwrites request headers
    Given the control plane is running
    And an upstream service is running
    And the gateway is running
    When I POST "/policies" on the control plane with JSON:
      """
      {
        "id": "add-header",
        "version": "1.0.0",
        "wasm_uri": "{{policy_add_header_wasm_uri}}",
        "sha256": "{{policy_add_header_sha256}}",
        "supported_stages": ["pre_upstream"],
        "config_schema": {
          "type": "object",
          "required": ["headers"],
          "properties": {
            "headers": {
              "type": "array",
              "items": {
                "type": "object",
                "required": ["name", "value", "overwrite"],
                "properties": {
                  "name": { "type": "string", "minLength": 1 },
                  "value": { "type": "string" },
                  "overwrite": { "type": "boolean" }
                },
                "additionalProperties": false
              },
              "minItems": 1
            }
          },
          "additionalProperties": false
        },
        "default_config": {
          "headers": [
            { "name": "x-policy-header", "value": "from-default", "overwrite": false }
          ]
        }
      }
      """
    Then the response status should be 201
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "policy-add-header",
        "match": { "path_prefix": "/v1/policy-add-header", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": [
          {
            "stage": "pre_upstream",
            "id": "add-header",
            "version": "1.0.0",
            "params": {
              "headers": [
                { "name": "x-policy-header", "value": "from-override", "overwrite": true },
                { "name": "x-extra-header", "value": "extra", "overwrite": false }
              ]
            }
          }
        ]
      }
      """
    Then the response status should be 201
    When I wait for the route "/v1/policy-add-header" to be available
    When I GET "/v1/policy-add-header" on the gateway with headers:
      """
      {
        "x-policy-header": "from-client"
      }
      """
    Then the response status should be 200
    And the response text should be "upstream-ok"
    When I GET "/debug/headers" on the upstream
    Then the response status should be 200
    And the JSON response should include:
      """
      {
        "x-policy-header": ["from-override"],
        "x-extra-header": ["extra"]
      }
      """

  Scenario: Unsupported policy executor fails closed with 500
    Given the control plane is running
    And an upstream service is running
    And the gateway is running
    When I POST "/policies" on the control plane with JSON:
      """
      {
        "id": "rewrite-anything",
        "version": "1.0.0",
        "wasm_uri": "{{policy_add_header_wasm_uri}}",
        "sha256": "{{policy_add_header_sha256}}",
        "supported_stages": ["pre_upstream"],
        "config_schema": { "type": "object" },
        "default_config": {}
      }
      """
    Then the response status should be 201
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "unsupported-policy-route",
        "match": { "path_prefix": "/v1/unsupported-policy", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": [
          {
            "stage": "pre_upstream",
            "id": "rewrite-anything",
            "version": "1.0.0",
            "params": {}
          }
        ]
      }
      """
    Then the response status should be 201
    When I GET "/v1/unsupported-policy" on the gateway
    Then the response status should be 500
