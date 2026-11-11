Name=AimAdapt Test - Last Lives
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]One slot plays three balls in turn, each with its own size. Three lives, and the time left scores.
AimTypeTag=Clicking
Timelimit=100.0
PlayerProfile=Gunner
AddedBots=Trio.rot
BotTeams=2
BotMaxLives=3
ScorePerKill=0.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
ScorePerTime=1.0
MapName=Last Lives.json
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
TimeBetweenShots=0.1
HitscanRadius=0.0
AAMode=0
[Bot Rotation Profile]
Name=Trio
ProfileNames=Small;Middle;Large
ProfileWeights=1.0;1.0;1.0
Randomized=false
[Bot Profile]
Name=Small
CharacterProfile=Small Ball
UseWeapons=false
[Bot Profile]
Name=Middle
CharacterProfile=Middle Ball
UseWeapons=false
[Bot Profile]
Name=Large
CharacterProfile=Large Ball
UseWeapons=false
[Character Profile]
Name=Small Ball
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=20.0
MainBBHeight=40.0
MainBBHasHead=false
EnemyBodyColor=X=1.000 Y=0.000 Z=0.000
MinRespawnDelay=0.2
MaxRespawnDelay=0.2
[Character Profile]
Name=Middle Ball
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=30.0
MainBBHeight=60.0
MainBBHasHead=false
EnemyBodyColor=X=0.000 Y=1.000 Z=0.000
MinRespawnDelay=0.2
MaxRespawnDelay=0.2
[Character Profile]
Name=Large Ball
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=40.0
MainBBHeight=80.0
MainBBHasHead=false
EnemyBodyColor=X=0.000 Y=0.000 Z=1.000
MinRespawnDelay=0.2
MaxRespawnDelay=0.2
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "800, 0, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
