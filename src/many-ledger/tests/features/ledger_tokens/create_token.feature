Feature: Create ledger Tokens

@tokens
Scenario: Creating a new token
	Given a ticker FBR
	And a name Foobar
	And a decimals of 9
	And an owner maa
	Given id 1 has 123 initial tokens
	And id 2 has 456 initial tokens
	When the token is created
	Then the token symbol is maa
	And the token ticker is FBR
	And the token name is Foobar
	And the token owner is maa
	And the token total supply is 579
	And the token circulating supply is 579
	And the token maximum supply has no maximum

@tokens
Scenario: Creating a new token without owner (owner is sender)
	Given a ticker FOO
	And no owner
	When the token is created
	Then the token ticker is FOO
	And the sender is the owner

@tokens
Scenario: Creating a new token removing the owner
	Given a ticker FOO
	And removing the owner
	When the token is created
	Then the token ticker is FOO
	And the owner is removed
