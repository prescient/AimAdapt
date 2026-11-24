Name=AimAdapt Test - Slow Taps
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]Three still balls on a wall, one click each, with the whole game at half speed and a timer of half the real time.
AimTypeTag=Clicking
Timelimit=10.0
Timescale=0.5
PlayerProfile=Gunner
AddedBots=Tap.bot;Tap.bot;Tap.bot
BotTeams=2;2;2
ScorePerKill=1.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Slow Taps.json
MapScale=4.0
InvinciblePlayer=true
BlockHealthbars=true
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
TimeBetweenShots=0.1
HitscanRadius=0.0
[Bot Profile]
Name=Tap
CharacterProfile=Tap Ball
UseWeapons=false
[Character Profile]
Name=Tap Ball
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=30.0
MainBBHeight=60.0
MainBBHasHead=false
EnemyBodyColor=X=0.300 Y=0.900 Z=0.400
MinRespawnDelay=0.2
MaxRespawnDelay=0.2
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "300, -200, -200", "rotation": "0, 0, 0", "scale": "0.5, 4, 4"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "250, -100, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "250, 0, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "250, 100, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
