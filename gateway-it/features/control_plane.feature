Feature: Control plane routes and policies

  Scenario: Register a policy and create a route
    Given the control plane is running
    When I register a policy with id "authn" and version "1.0.0"
    Then the response status should be 201
    When I create a route with id "users"
    Then the response status should be 201
    When I list routes
    Then the response should include route "users"
