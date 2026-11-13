Name=AimAdapt Test - Trail Pair
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]Two balls strafe in a lane. The map has a path of waypoints, which their dodge profile ignores.
AimTypeTag=Tracking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Strider.bot;Strider.bot
BotTeams=2;2
ScorePerKill=5.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Trail Pair.json
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
Name=Strider
CharacterProfile=Strider Ball
DodgeProfileNames=Side Weave
DodgeProfileWeights=1.0
UseWeapons=false
SeeThroughWalls=false
[Character Profile]
Name=Strider Ball
MaxHealth=3.0
MaxSpeed=500.0
Acceleration=3000.0
Friction=8.0
MainBBType=Spheroid
MainBBRadius=40.0
MainBBHeight=80.0
MainBBHasHead=false
EnemyBodyColor=X=0.900 Y=0.700 Z=0.100
[Dodge Profile]
Name=Side Weave
ToggleLeftRight=true
MinLRTimeChange=0.5
MaxLRTimeChange=0.9
WaypointLogic=Ignore
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1500, -250", "rotation": "0, 0, 0", "scale": "30, 30, 1"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -900, -250", "rotation": "0, 0, 0", "scale": "30, 1, 8"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, 800, -250", "rotation": "0, 0, 0", "scale": "30, 1, 8"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1000, 0, -150", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}, {"name": "Path", "value": "lane a,lane b,lane c,lane d"}, {"name": "LoopingPath", "value": true}]},
 {"type": "gameObject", "name": "Waypoint", "location": "1000, -400, -150", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "Name", "value": "lane a"}, {"name": "BotPauseTimeMin", "value": 0.0}, {"name": "BotPauseTimeMax", "value": 0.5}]},
 {"type": "gameObject", "name": "Waypoint", "location": "1000, 400, -150", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "Name", "value": "lane b"}]},
 {"type": "gameObject", "name": "Waypoint", "location": "1200, 400, -150", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "Name", "value": "lane c"}]},
 {"type": "gameObject", "name": "Waypoint", "location": "1200, -400, -150", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "Name", "value": "lane d"}]}
]}
