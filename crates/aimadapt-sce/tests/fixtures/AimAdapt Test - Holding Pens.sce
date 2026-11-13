Name=AimAdapt Test - Holding Pens
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]Three balls of two kinds. Two small hurt cubes far outside the room hold a ball each kind can start in, and the cube's cooldown decides when it lets it go.
AimTypeTag=Clicking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Pen Red.bot;Pen Blue.bot;Pen Red.bot
BotTeams=2;2;2
ScorePerKill=10.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Holding Pens.json
MapScale=1.5
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
Name=Pen Red
CharacterProfile=Red Ball
UseWeapons=false
[Bot Profile]
Name=Pen Blue
CharacterProfile=Blue Ball
UseWeapons=false
[Character Profile]
Name=Red Ball
MaxHealth=5.0
MinRespawnDelay=0.1
MainBBType=Spheroid
MainBBRadius=30.0
MainBBHeight=60.0
MainBBHasHead=false
EnemyBodyColor=X=0.900 Y=0.200 Z=0.200
[Character Profile]
Name=Blue Ball
MaxHealth=5.0
MinRespawnDelay=0.1
MainBBType=Spheroid
MainBBRadius=45.0
MainBBHeight=90.0
MainBBHasHead=false
EnemyBodyColor=X=0.200 Y=0.300 Z=0.900
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1500, -250", "rotation": "0, 0, 0", "scale": "30, 30, 1"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnVolume", "location": "800, -300, -100", "rotation": "0, 0, 0", "scale": "3, 6, 2", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "Hurt", "location": "2000, 3000, 0", "rotation": "0, 0, 0", "scale": "2, 2, 2", "properties": [{"name": "Kill", "value": false}, {"name": "Damage", "value": 40.0}, {"name": "Cooldown", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "2100, 3100, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 3.0}, {"name": "PermittedCharacterProfiles", "value": "Red Ball"}]},
 {"type": "gameObject", "name": "Hurt", "location": "2000, -3200, 0", "rotation": "0, 0, 0", "scale": "2, 2, 2", "properties": [{"name": "Kill", "value": false}, {"name": "Damage", "value": 40.0}, {"name": "Cooldown", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "2100, -3100, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 3.0}, {"name": "PermittedCharacterProfiles", "value": "Blue Ball"}]}
]}
