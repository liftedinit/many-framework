Feature: Create ledger Tokens

@tokens
Scenario: Creating a new token as myself
	Given a ticker FBR
	And a name Foobar
	And a decimals of 9
	And myself as owner
	Given id 1 has 123 initial tokens
	And id 2 has 456 initial tokens
	When the token is created
	Then the token symbol is a subresource
	And the token ticker is FBR
	And the token name is Foobar
	And the token owner is myself
	And the token total supply is 579
	And the token circulating supply is 579
	And the token maximum supply has no maximum

@tokens
Scenario: Creating a new token as someone random
	Given a random owner
	Then creating the token as myself fails with unauthorized

@tokens
Scenario: Creating a new token as anonymous
	Given an anonymous owner
	Then creating the token as myself fails with unauthorized

@tokens
Scenario: Creating a new token on behalf of an account I'm not part of
	Given a token account where id 5 is the owner
	And setting the account as the owner
	Then creating the token as myself fails with missing permission

@tokens
Scenario: Creating a new token on behalf of an account I'm the Owner of
	Given a token account I'm part of as Owner
	And setting the account as the owner
	When the token is created
	Then the token owner is the account

@tokens
Scenario: Creating a new token on behalf of an account I'm part of with token creation permission
	Given a token account id 5 is part of with token creation permission
	And setting the account as the owner
	When the token is created as id 5
	Then the token owner is the account

@tokens
Scenario: Creating a new token on behalf of an account I'm part of without token creation permission
	Given a token account id 6 is part of without token creation permission
	And setting the account as the owner
	Then creating the token as id 6 fails with missing permission

@tokens
Scenario: Creating a new token without owner (owner is sender)
	Given a ticker FOO
	And no owner
	When the token is created
	Then the token ticker is FOO
	And the sender is the owner

@tokens
Scenario: Creating a new token, removing the owner
	Given a ticker FOO
	And removing the owner
	When the token is created
	Then the token ticker is FOO
	And the owner is removed
