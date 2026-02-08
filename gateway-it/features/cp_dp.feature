Feature: Control plane to data plane routing

  Scenario: A route created in control plane is applied in the gateway
    Given the control plane is running
    And an upstream service is running
    And the gateway is running
    When I POST "/routes" on the control plane with JSON:
      """
      {
        "id": "users",
        "match": { "path_prefix": "/v1/users", "method": ["GET"] },
        "upstreams": [
          { "url": "{{upstream_url}}" }
        ],
        "policies": []
      }
      """
    Then the response status should be 201
    When I wait for the route "/v1/users" to be available
    When I GET "/v1/users" on the gateway
    Then the response status should be 200
    And the response text should be "upstream-ok"
