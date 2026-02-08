Feature: Deterministic route matching

  Scenario: Longest path prefix wins over generic prefix
    Given the control plane is running
    And an upstream service is running
    And the gateway is running
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "det-prefix-generic",
        "match": { "path_prefix": "/deterministic/users", "method": ["GET"] },
        "upstreams": [
          { "url": "http://10.0.0.12:8080" }
        ],
        "policies": []
      }
      """
    Then the response status should be 201
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "det-prefix-specific",
        "match": { "path_prefix": "/deterministic/users/profile", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": []
      }
      """
    Then the response status should be 201
    When I wait for the route "/deterministic/users/profile" to be available
    When I GET "/deterministic/users/profile" on the gateway
    Then the response status should be 200
    And the response text should be "upstream-ok"

  Scenario: Method specific route wins over wildcard method
    Given the control plane is running
    And an upstream service is running
    And the gateway is running
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "det-method-any",
        "match": { "path_prefix": "/deterministic/method" },
        "upstreams": [
          { "url": "http://10.0.0.12:8080" }
        ],
        "policies": []
      }
      """
    Then the response status should be 201
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "det-method-get",
        "match": { "path_prefix": "/deterministic/method", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": []
      }
      """
    Then the response status should be 201
    When I wait for the route "/deterministic/method" to be available
    When I GET "/deterministic/method" on the gateway
    Then the response status should be 200
    And the response text should be "upstream-ok"
