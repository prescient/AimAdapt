Name=AimAdapt Test - Loose Choke
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]Pellet Yard with a count that isn't whole and a pattern that spreads.
AimTypeTag=Clicking
Timelimit=30.0
PlayerProfile=Gunner
AddedBots=Dot.bot;Dot.bot;Dot.bot
BotTeams=2;2;2
ScorePerKill=0.0
ScorePerHit=0.0
ScorePerDamage=1.0
ScoreLossPerMiss=0.0
MapName=Pellet Yard.json
MapScale=4.0
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
WeaponProfileNames=Scatter;;;
[Weapon Profile]
Name=Scatter
Type=Hitscan
Category=SemiAuto
ShotsPerClick=2.5
DamagePerShot=5.0
TimeBetweenShots=0.4
UsePerBulletSpread=true
SpreadStationaryVelocity=25.0
SpreadDecayDelay=0.2
SpreadSSH=0.4,0.6,0.2,3.5
SpreadSSA=0.4,0.6,0.2,3.5
SpreadSCH=0.4,0.6,0.2,3.5
SpreadSCA=0.4,0.6,0.2,3.5
SpreadMSH=0.4,0.6,0.2,3.5
SpreadMSA=0.4,0.6,0.2,3.5
SpreadMCH=0.4,0.6,0.2,3.5
SpreadMCA=0.4,0.6,0.2,3.5
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
MaxHealth=50.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=30.0
MainBBHeight=60.0
MainBBHasHead=false
EnemyBodyColor=X=1.000 Y=0.500 Z=0.000
BlockedSpawnRadius=150.0
SpawnOffsetMin=X=0.000 Y=-50.000 Z=50.000
SpawnOffsetMax=X=0.000 Y=50.000 Z=-50.000
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "300, -200, -200", "rotation": "0, 0, 0", "scale": "0.5, 4, 4"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "250, -100, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "250, 0, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "250, 100, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
