Name=AimAdapt Test - Slant Walk
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]One ball on a floor weaves sideways and in and out at once, with a forward bias.
AimTypeTag=Tracking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Weaver.bot
BotTeams=2
ScorePerKill=5.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Slant Walk.json
MapScale=1.0
InvinciblePlayer=true
BlockHealthbars=true
[Character Profile]
Name=Gunner
MaxHealth=1.0
MaxSpeed=0.0
CameraOffset=X=0.000 Y=0.000 Z=0.000
SpawnOffsetMin=X=0.000 Y=0.000 Z=0.000
SpawnOffsetMax=X=0.000 Y=0.000 Z=0.000
WeaponProfileNames=Tapper;;;
[Weapon Profile]
Name=Tapper
Type=Hitscan
Category=FullyAuto
DamagePerShot=1.0
TimeBetweenShots=0.1
HitscanRadius=0.0
AAMode=0
[Bot Profile]
Name=Weaver
CharacterProfile=Weaver Ball
DodgeProfileNames=Slant Weave
DodgeProfileWeights=1.0
UseWeapons=false
[Character Profile]
Name=Weaver Ball
MaxHealth=3.0
MaxSpeed=300.0
Acceleration=3000.0
Friction=8.0
ForwardSpeedBias=0.25
BackSpeedMult=0.8
StrafeSpeedMult=0.5
MainBBType=Spheroid
MainBBRadius=40.0
MainBBHeight=80.0
MainBBHasHead=false
EnemyBodyColor=X=0.900 Y=0.300 Z=0.700
[Dodge Profile]
Name=Slant Weave
ToggleLeftRight=true
MinLRTimeChange=0.6
MaxLRTimeChange=0.9
ToggleForwardBack=true
MinFBTimeChange=0.7
MaxFBTimeChange=1.0
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "0, -300, -250", "rotation": "0, 0, 0", "scale": "30, 30, 1"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1000, 0, -150", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
