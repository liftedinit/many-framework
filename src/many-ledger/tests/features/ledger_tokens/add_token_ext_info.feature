Feature: Add token extended info

@token
Scenario: Add token memo extended info
	Given a default token
	And a memo "Oh my god, it's full of stars"
	When I add the extended info to the token
	Then the token has the memo "Oh my god, it's full of stars"

@token
Scenario: Add token unicode char logo extended info
	Given a default token
	And an unicode logo ∑
	When I add the extended info to the token
	Then the token has the unicode logo ∑

@token
Scenario: Add token image logo extended info
	Given a default token
	And a png image logo '010203'
	When I add the extended info to the token
	Then the token has the png image logo '010203'
