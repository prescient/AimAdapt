Name=AimAdapt Test - Kill Gate
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]Two balls strafe in a lane. A thin teleporter across part of it sends a ball into a pit that kills it at once.
AimTypeTag=Tracking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Strider.bot;Strider.bot
BotTeams=2;2
ScorePerKill=5.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Gate Lane.json
MapScale=2.0
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
Name=Strider
CharacterProfile=Strider Ball
DodgeProfileNames=Side Weave
DodgeProfileWeights=1.0
UseWeapons=false
SeeThroughWalls=false
[Character Profile]
Name=Strider Ball
MaxHealth=20.0
MaxSpeed=500.0
Acceleration=3000.0
Friction=8.0
MainBBType=Spheroid
MainBBRadius=40.0
MainBBHeight=80.0
MainBBHasHead=false
EnemyBodyColor=X=0.300 Y=0.800 Z=0.600
[Dodge Profile]
Name=Side Weave
ToggleLeftRight=true
MinLRTimeChange=0.5
MaxLRTimeChange=0.9
WaypointLogic=Ignore
[Map Data]
{"version": "0.0.7", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1500, -250", "rotation": "0, 0, 0", "scale": "30, 30, 1"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -900, -250", "rotation": "0, 0, 0", "scale": "30, 1, 8"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, 800, -250", "rotation": "0, 0, 0", "scale": "30, 1, 8"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1000, 0, -150", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "Teleporter", "location": "1000, 100, -50", "rotation": "0, 0, 0", "scale": "6, 0.2, 3", "properties": [{"name": "Target", "value": "pit"}, {"name": "TeleportDelay", "value": 0.0}]},
 {"type": "gameObject", "name": "Waypoint", "location": "1000, 5000, 5000", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "Name", "value": "pit"}]},
 {"type": "gameObject", "name": "Hurt", "location": "1000, 5000, 5000", "rotation": "0, 0, 0", "scale": "4, 4, 4", "properties": [{"name": "Kill", "value": false}, {"name": "Damage", "value": 100000.0}, {"name": "Cooldown", "value": 0.0}]}
]}
