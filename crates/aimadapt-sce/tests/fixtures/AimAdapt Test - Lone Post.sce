Name=AimAdapt Test - Lone Post
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]One ball stands still. Its dodge profile follows waypoints, but no spawn point names a path.
AimTypeTag=Clicking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Post.bot
BotTeams=2
ScorePerKill=5.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Lone Post.json
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
Category=SemiAuto
DamagePerShot=1.0
TimeBetweenShots=0.2
HitscanRadius=0.0
AAMode=0
[Bot Profile]
Name=Post
CharacterProfile=Post Ball
DodgeProfileNames=Stand Guard
DodgeProfileWeights=1.0
UseWeapons=false
SeeThroughWalls=false
[Character Profile]
Name=Post Ball
MaxHealth=50.0
MaxSpeed=500.0
Acceleration=3000.0
Friction=8.0
MainBBType=Spheroid
MainBBRadius=40.0
MainBBHeight=80.0
MainBBHasHead=false
EnemyBodyColor=X=0.300 Y=0.900 Z=0.400
[Dodge Profile]
Name=Stand Guard
ToggleLeftRight=false
ToggleForwardBack=false
WaypointLogic=FollowAimAtWaypoint
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1500, -250", "rotation": "0, 0, 0", "scale": "30, 30, 1"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -900, -250", "rotation": "0, 0, 0", "scale": "30, 1, 8"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, 800, -250", "rotation": "0, 0, 0", "scale": "30, 1, 8"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1000, 0, -150", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}, {"name": "Path", "value": ""}, {"name": "LoopingPath", "value": false}]},
 {"type": "gameObject", "name": "Waypoint", "location": "1000, -400, -150", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "Name", "value": "spare a"}]},
 {"type": "gameObject", "name": "Waypoint", "location": "1000, 400, -150", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "Name", "value": "spare b"}]}
]}
