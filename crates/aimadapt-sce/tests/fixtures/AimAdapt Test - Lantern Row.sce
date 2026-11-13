Name=AimAdapt Test - Lantern Row
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]Two still balls on a plain map with a window and two lanterns that block shots.
AimTypeTag=Clicking
Timelimit=30.0
PlayerProfile=Gunner
AddedBots=Dot.bot;Dot.bot
BotTeams=2;2
ScorePerKill=10.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Lantern Row.json
MapScale=2.0
InvinciblePlayer=true
BlockHealthbars=true
LockFOVRange=true
LockedFOVMin=100.0
LockedFOVMax=120.0
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
Name=Dot
CharacterProfile=Dot Ball
UseWeapons=false
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Character Profile]
Name=Dot Ball
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=30.0
MainBBHeight=60.0
MainBBHasHead=false
EnemyBodyColor=X=1.000 Y=0.500 Z=0.000
SpawnOffsetMin=X=0.000 Y=0.000 Z=0.000
SpawnOffsetMax=X=0.000 Y=0.000 Z=0.000
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnVolume", "location": "600, -100, -50", "rotation": "0, 0, 0", "scale": "0.1, 2, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "prop", "name": "Window", "location": "1000, 0, -300", "rotation": "0, 0, 0", "scale": "0.03, 30, 2", "properties": [{"name": "EnableCollision", "value": true}]},
 {"type": "prop", "name": "Lantern", "location": "300, 500, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "EnableCollision", "value": true}]},
 {"type": "prop", "name": "Lantern", "location": "300, -500, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "EnableCollision", "value": true}]}
]}
