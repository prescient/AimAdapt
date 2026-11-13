Name=AimAdapt Test - Ledge Mote
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]One speck of a ball under a far head, which falls onto a block beside a wall.
AimTypeTag=Clicking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Mote.bot
BotTeams=2
ScorePerKill=100.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Ledge Mote.json
MapScale=1.0
InvinciblePlayer=true
[Character Profile]
Name=Gunner
MaxHealth=1.0
MaxSpeed=0.0
CameraOffset=X=0.000 Y=0.000 Z=0.000
WeaponProfileNames=Mote Pistol;;;
[Weapon Profile]
Name=Mote Pistol
Type=Hitscan
Category=SemiAuto
DamagePerShot=100.0
TimeBetweenShots=0.2
HitscanRadius=0.0
AAMode=0
[Bot Profile]
Name=Mote
CharacterProfile=Mote Ball
UseWeapons=false
[Character Profile]
Name=Mote Ball
MaxHealth=100.0
MaxSpeed=0.0
Gravity=1.0
MainBBType=Spheroid
MainBBRadius=0.3
MainBBHeight=0.6
MainBBHasHead=true
MainBBHeadRadius=40.0
MainBBHeadOffset=200.0
EnemyHeadColor=X=0.900 Y=0.400 Z=0.800
MinRespawnDelay=0.5
MaxRespawnDelay=0.5
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "1000, -300, -400", "rotation": "0, 0, 0", "scale": "6, 6, 3"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "1600, -300, -400", "rotation": "0, 0, 0", "scale": "1, 6, 7"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 20", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1510, 0, -100", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
