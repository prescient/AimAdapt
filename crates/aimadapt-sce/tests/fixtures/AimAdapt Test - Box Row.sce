Name=AimAdapt Test - Box Row
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]Three still box bots in a row.
AimTypeTag=Clicking
Timelimit=30.0
PlayerProfile=Gunner
AddedBots=Crate.bot;Crate.bot;Crate.bot
BotTeams=2;2;2
ScorePerKill=10.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Box Row.json
MapScale=1.0
InvinciblePlayer=true
[Character Profile]
Name=Gunner
MaxHealth=1.0
MaxSpeed=0.0
WeaponProfileNames=Tapper;;;
[Weapon Profile]
Name=Tapper
Type=Hitscan
Category=SemiAuto
DamagePerShot=1.0
TimeBetweenShots=0.2
HitscanRadius=0.0
[Bot Profile]
Name=Crate
CharacterProfile=Crate Body
UseWeapons=false
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Character Profile]
Name=Crate Body
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Cuboid
MainBBRadius=35.0
MainBBHeight=150.0
MainBBHasHead=false
EnemyBodyColor=X=0.200 Y=0.600 Z=1.000
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "1100, -600, -300", "rotation": "0, 0, 0", "scale": "0.5, 12, 6"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "800, -200, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "800, 0, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "800, 200, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
