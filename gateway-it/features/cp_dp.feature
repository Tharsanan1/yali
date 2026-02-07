Feature: Control plane to data plane routing

  Scenario: A route created in control plane is applied in the gateway
    Given the control plane is running
    And an upstream service is running
    And the gateway is running
    When I create a route pointing to the upstream
    Then the response status should be 201
    When I wait for the route "/v1/users" to be available
    Then a request to "/v1/users" should return "upstream-ok"
