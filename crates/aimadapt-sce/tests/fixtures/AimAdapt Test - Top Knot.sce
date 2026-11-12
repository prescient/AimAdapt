Name=AimAdapt Test - Top Knot
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]Three still capsules with heads, a damage efficiency score.
AimTypeTag=Clicking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Knot.bot;Knot.bot;Knot.bot
BotTeams=2;2;2
ScorePerKill=100.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
ScoreMultDamageEfficiency=true
MapName=Top Knot.json
MapScale=1.0
InvinciblePlayer=true
[Character Profile]
Name=Gunner
MaxHealth=1.0
MaxSpeed=0.0
CameraOffset=X=0.000 Y=0.000 Z=0.000
WeaponProfileNames=Knot Pistol;;;
[Weapon Profile]
Name=Knot Pistol
Type=Hitscan
Category=SemiAuto
DamagePerShot=25.0
TimeBetweenShots=0.2
HitscanRadius=0.0
AAMode=0
HeadshotCapable=true
HeadshotMultiplier=4.0
[Bot Profile]
Name=Knot
CharacterProfile=Knot Capsule
UseWeapons=false
[Character Profile]
Name=Knot Capsule
MaxHealth=100.0
MaxSpeed=0.0
MainBBType=Cylindrical
MainBBRadius=40.0
MainBBHeight=180.0
MainBBHasHead=true
MainBBHeadRadius=25.0
MainBBHeadOffset=-10.0
HeadshotOnly=false
EnemyHeadColor=X=0.900 Y=0.800 Z=0.200
BlockedSpawnRadius=100.0
MinRespawnDelay=0.5
MaxRespawnDelay=0.5
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "1200, -600, -400", "rotation": "0, 0, 0", "scale": "0.5, 12, 8"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "800, -150, -180", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "800, 0, -180", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "800, 150, -180", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
