Feature: Update ledger Tokens

@tokens
Scenario: Updating a token's ticker
	Given a default token
	And a new token ticker ABC
	When I update the token
	Then the token new ticker is ABC

@tokens
Scenario: Updating a token's name
	Given a default token
	And a new token name Supercalifragilisticexpialidocious
	When I update the token
	Then the token new name is Supercalifragilisticexpialidocious

@tokens
Scenario: Updating a token's decimal
	Given a default token
	And a new token decimal 16
	When I update the token
	Then the token new decimal is 16

@tokens
Scenario: Updating a token's owner
	Given a default token
	And a new token owner maem7b3bkzipk4dluaxjlw2pnwjll4rggqn4akimt3xhjhja55
	When I update the token
	Then the token new owner is maem7b3bkzipk4dluaxjlw2pnwjll4rggqn4akimt3xhjhja55

@tokens
Scenario: Removing a token's owner
	Given a default token
	And removing the token owner
	When I update the token
	Then the token owner is removed
