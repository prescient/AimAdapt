Name=AimAdapt Test - Old Room
PlayerCharacters=Player
BotCharacters=still.bot
IsChallenge=true
Timelimit=30.0
PlayerProfile=Player
AddedBots=still.bot;still.bot
BotTeams=0;0
ScorePerKill=1.0
MapName=old room.map
MapScale=3.0
InvinciblePlayer=true
Timescale=0.9
Description=Made up for AimAdapt's tests.
GameVersion=1.0.8.0

[Bot Profile]
Name=still
DodgeProfileNames=Stand
DodgeProfileWeights=1.0
CharacterProfile=box target
UseWeapons=false
NoDodging=true

[Character Profile]
Name=Player
MaxHealth=100.0
WeaponProfileNames=Lobber;;;;;;;
MaxSpeed=0.0

[Character Profile]
Name=box target
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Cuboid
MainBBHeight=64.0
MainBBRadius=32.0
MainBBHasHead=true

[Dodge Profile]
Name=Stand
ToggleLeftRight=false

[Weapon Profile]
Name=Lobber
Type=Projectile
Category=SemiAuto
DamagePerShot=1.0
Explosive=false

[Map Data]
reflex map version 8
global
	entity
		type WorldSpawn
		String32 targetGameOverCamera end
		UInt8 playersMax 16
	brush
		vertices
			-512.000000 0.000000 -512.000000
			512.000000 0.000000 -512.000000
			512.000000 0.000000 512.000000
			-512.000000 0.000000 512.000000
			-512.000000 -32.000000 -512.000000
			512.000000 -32.000000 -512.000000
			512.000000 -32.000000 512.000000
			-512.000000 -32.000000 512.000000
		faces
			0.000000 0.000000 1.000000 1.000000 0.000000 0 1 2 3 0x00000000 
			0.000000 0.000000 1.000000 1.000000 0.000000 7 6 5 4 0x00000000 
			0.000000 0.000000 1.000000 1.000000 0.000000 1 0 4 5 0x00000000 internal/editor/textures/plain
			0.000000 0.000000 1.000000 1.000000 0.000000 2 1 5 6 0x00000000 
			0.000000 0.000000 1.000000 1.000000 0.000000 3 2 6 7 0x00000000 
			0.000000 0.000000 1.000000 1.000000 0.000000 0 3 7 4 0x00000000 
	entity
		type PlayerSpawn
		Vector3 position 0.000000 16.000000 -400.000000
		Vector3 angles 180.000000 0.000000 0.000000
		Bool8 teamB 0
		Bool8 initialSpawn 1
	entity
		type PlayerSpawn
		Vector3 position -100.000000 64.000000 300.000000
		Bool8 TeamA 0
	entity
		type PlayerSpawn
		Vector3 position 100.000000 64.000000 300.000000
		Bool8 teamA 0
	entity
		type Target
		Vector3 position 0.000000 200.000000 0.000000
		String32 name end
	entity
		type Teleporter
		Vector3 position 400.000000 0.000000 400.000000
		String32 target end
