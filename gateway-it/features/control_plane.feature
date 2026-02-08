Feature: Control plane routes and policies

  Scenario: Register a policy and create a route
    Given the control plane is running
    When I POST "/policies" on the control plane with JSON:
      """
      {
        "id": "authn",
        "version": "1.0.0",
        "wasm_uri": "file:///policies/authn.wasm",
        "sha256": "deadbeef",
        "config": { "mode": "jwt", "issuer": "example" }
      }
      """
    Then the response status should be 201
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "users",
        "match": { "path_prefix": "/v1/users", "method": ["GET", "POST"] },
        "lb": "round_robin",
        "failover": { "enabled": true, "max_failovers": 1, "retry_on": ["connect_failure", "5xx"], "per_try_timeout_ms": 1000 },
        "upstreams": [
          { "url": "http://10.0.0.12:8080", "weight": 100, "priority": 0 }
        ],
        "policies": [
          { "stage": "pre_route", "id": "authn", "version": "1.0.0" }
        ]
      }
      """
    Then the response status should be 201
    When I GET "/routes" on the control plane
    Then the JSON response should include:
      """
      { "id": "users" }
      """
