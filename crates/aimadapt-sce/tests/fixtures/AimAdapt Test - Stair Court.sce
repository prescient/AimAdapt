Name=AimAdapt Test - Stair Court
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]Two kinds of still ball drop onto a floor from different heights, and a walker appears just above it.
AimTypeTag=Clicking
Timelimit=30.0
PlayerProfile=Gunner
AddedBots=Pebble.bot;Boulder.bot;Strider.bot
BotTeams=2;2;2
ScorePerKill=10.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Stair Court.json
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
Name=Pebble
CharacterProfile=Pebble Ball
UseWeapons=false
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Character Profile]
Name=Pebble Ball
MaxHealth=1.0
MaxSpeed=0.0
Gravity=1.0
SpawnOffsetMin=X=0.000 Y=-50.000 Z=0.000
SpawnOffsetMax=X=0.000 Y=50.000 Z=0.000
MainBBType=Spheroid
MainBBRadius=20.0
MainBBHeight=40.0
MainBBHasHead=false
EnemyBodyColor=X=0.900 Y=0.700 Z=0.200
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Bot Profile]
Name=Boulder
CharacterProfile=Boulder Ball
UseWeapons=false
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Character Profile]
Name=Boulder Ball
MaxHealth=1.0
MaxSpeed=0.0
Gravity=1.0
SpawnOffsetMin=X=0.000 Y=-300.000 Z=0.000
SpawnOffsetMax=X=100.000 Y=300.000 Z=50.000
MainBBType=Spheroid
MainBBRadius=60.0
MainBBHeight=120.0
MainBBHasHead=false
EnemyBodyColor=X=0.400 Y=0.400 Z=0.900
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Bot Profile]
Name=Strider
CharacterProfile=Strider Body
UseWeapons=false
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Character Profile]
Name=Strider Body
MaxHealth=1.0
MaxSpeed=300.0
Gravity=1.0
StepUpHeight=75.0
MainBBType=Cylindrical
MainBBRadius=30.0
MainBBHeight=170.0
MainBBHasHead=false
EnemyBodyColor=X=0.300 Y=0.900 Z=0.400
MinRespawnDelay=0.1
MaxRespawnDelay=0.3
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "300, -1000, -300", "rotation": "0, 0, 0", "scale": "20, 20, 2"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "800, 0, 200", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}, {"name": "PermittedCharacterProfiles", "value": "Pebble Ball"}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "800, 300, -135", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}, {"name": "PermittedCharacterProfiles", "value": "Strider Body"}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "800, -600, 200", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}, {"name": "PermittedCharacterProfiles", "value": "Boulder Ball"}]}
]}
