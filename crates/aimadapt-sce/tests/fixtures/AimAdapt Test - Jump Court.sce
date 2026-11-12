Name=AimAdapt Test - Jump Court
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]A hopping ball with an air jump, a ball with a jetpack and a tank, and a flying ball flown by its jump and crouch keys.
AimTypeTag=Clicking
Timelimit=30.0
PlayerProfile=Gunner
AddedBots=Hopper.bot;Floater.bot;Glider.bot
BotTeams=2;2;2
ScorePerKill=10.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Jump Court.json
MapScale=1.0
InvinciblePlayer=true
[Character Profile]
Name=Gunner
MaxHealth=1.0
MaxSpeed=0.0
WeaponProfileNames=Tapper;;;
[Weapon Profile]
Name=Tapper
Type=Hitscan
Category=SemiAuto
DamagePerShot=1.0
TimeBetweenShots=0.2
HitscanRadius=0.0
[Bot Profile]
Name=Hopper
CharacterProfile=Hopper Ball
DodgeProfileNames=Hop
DodgeProfileWeights=1.0
UseWeapons=false
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Character Profile]
Name=Hopper Ball
MaxHealth=1.0
MaxSpeed=0.0
Gravity=1.0
JumpVelocity=300.0
AirJumpCount=1.0
AirJumpVelocity=350.0
StepUpHeight=75.0
CanPogoJump=true
AllowBufferedJumps=true
MainBBType=Spheroid
MainBBRadius=25.0
MainBBHeight=50.0
MainBBHasHead=false
EnemyBodyColor=X=0.900 Y=0.500 Z=0.200
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Dodge Profile]
Name=Hop
ToggleLeftRight=false
JumpFrequency=0.6
MinJumpTime=0.6
MaxJumpTime=1.2
[Bot Profile]
Name=Floater
CharacterProfile=Floater Ball
DodgeProfileNames=Float
DodgeProfileWeights=1.0
UseWeapons=false
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Character Profile]
Name=Floater Ball
MaxHealth=1.0
MaxSpeed=0.0
Gravity=1.0
JumpVelocity=250.0
HasJetpack=true
JetpackThrust=1500.0
JetpackMaxZVelocity=250.0
JetpackAirControlWithThrust=0.5
JetpackActivationDelay=0.4
JetpackFullFuelTime=2.5
JetpackFuelIncPerSec=0.5
JetpackFuelRegensInAir=false
MainBBType=Spheroid
MainBBRadius=30.0
MainBBHeight=60.0
MainBBHasHead=false
EnemyBodyColor=X=0.300 Y=0.600 Z=0.900
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Dodge Profile]
Name=Float
ToggleLeftRight=false
JumpFrequency=0.7
MinJumpTime=0.8
MaxJumpTime=1.6
[Bot Profile]
Name=Glider
CharacterProfile=Glider Ball
DodgeProfileNames=Glide
DodgeProfileWeights=1.0
UseWeapons=false
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Character Profile]
Name=Glider Ball
MaxHealth=1.0
MaxSpeed=0.0
Gravity=0.0
IsFlyer=true
FlightVelocityUp=200.0
FlightVelocityDown=300.0
FlightAccelUp=600.0
FlightAccelDown=900.0
CanCrouch=true
IsFlyUpOnJumpAndCrouch=true
MainBBType=Spheroid
MainBBRadius=20.0
MainBBHeight=40.0
MainBBHasHead=false
EnemyBodyColor=X=0.400 Y=0.900 Z=0.400
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Dodge Profile]
Name=Glide
ToggleLeftRight=false
AlterateJumpCrouchInput=false
JumpFrequency=0.5
MinJumpTime=0.5
MaxJumpTime=1.0
CrouchInAirFrequency=0.5
MinCrouchTime=0.4
MaxCrouchTime=0.8
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "300, -1500, -300", "rotation": "0, 0, 0", "scale": "20, 30, 2"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "900, -500, -100", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}, {"name": "PermittedCharacterProfiles", "value": "Hopper Ball"}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "900, 0, -100", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}, {"name": "PermittedCharacterProfiles", "value": "Floater Ball"}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "900, 500, 100", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}, {"name": "PermittedCharacterProfiles", "value": "Glider Ball"}]}
]}
