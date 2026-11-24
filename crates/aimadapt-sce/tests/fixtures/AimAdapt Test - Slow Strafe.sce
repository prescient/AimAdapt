Name=AimAdapt Test - Slow Strafe
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]One sturdy ball strafes left and right in front of a weapon held down, with the game at 80% speed and a timer of 80% of the real time.
AimTypeTag=Tracking
Timelimit=16.0
Timescale=0.8
PlayerProfile=Gunner
AddedBots=Swayer.bot
BotTeams=2
ScorePerKill=0.0
ScorePerHit=0.0
ScorePerDamage=1.0
ScoreLossPerMiss=0.0
MapName=Slow Strafe.json
MapScale=1.0
InvinciblePlayer=true
BlockHealthbars=true
[Character Profile]
Name=Gunner
MaxHealth=1.0
MaxSpeed=0.0
WeaponProfileNames=Stream;;;
[Weapon Profile]
Name=Stream
Type=Hitscan
Category=FullyAuto
DamagePerShot=1.0
TimeBetweenShots=0.01
HitscanRadius=0.0
[Bot Profile]
Name=Swayer
CharacterProfile=Swayer Ball
DodgeProfileNames=Sway
DodgeProfileWeights=1.0
UseWeapons=false
SeeThroughWalls=false
[Character Profile]
Name=Swayer Ball
MaxHealth=100000.0
MaxSpeed=500.0
Acceleration=4000.0
Friction=8.0
MainBBType=Spheroid
MainBBRadius=50.0
MainBBHeight=100.0
MainBBHasHead=false
EnemyBodyColor=X=1.000 Y=0.800 Z=0.200
[Dodge Profile]
Name=Sway
ToggleLeftRight=true
MinLRTimeChange=0.6
MaxLRTimeChange=1.0
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1500, -250", "rotation": "0, 0, 0", "scale": "30, 30, 1"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1200, 0, -150", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
