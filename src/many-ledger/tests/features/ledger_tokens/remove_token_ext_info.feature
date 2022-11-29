Feature: Remove token extended info

@token
Scenario: Remove token memo extended info
	Given a default token
	And the token has a memo
	When I remove the memo
	Then the token has no memo

@token
Scenario: Remove token logo extended info
	Given a default token
	And the token has a logo
	When I remove the logo
	Then the token has no logo

@token
Scenario: Remove token memo and logo extended info
	Given a default token
	And the token has a memo
	And the token has a logo
	When I remove the memo
	And I remove the logo
	Then the token has no memo
	And the token has no logo
