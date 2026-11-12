Name=AimAdapt Test - Quiet Guns
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]Two kinds of ball switch places on a wall. Each carries a gun but is told not to aim, so it never fires. Your own character is invincible.
AimTypeTag=Clicking
Timelimit=45.0
PlayerProfile=Marksman
AddedBots=Duo.rot;Duo.rot
BotTeams=2;2
ScorePerKill=5.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Quiet Guns.json
MapScale=3.0
InvinciblePlayer=false
BlockHealthbars=true
[Character Profile]
Name=Marksman
MaxHealth=100.0
MaxSpeed=0.0
InvinciblePlayer=true
CameraOffset=X=0.000 Y=0.000 Z=0.000
SpawnOffsetMin=X=0.000 Y=0.000 Z=0.000
SpawnOffsetMax=X=0.000 Y=0.000 Z=0.000
WeaponProfileNames=Needle;;;
[Weapon Profile]
Name=Needle
Type=Hitscan
Category=SemiAuto
DamagePerShot=10.0
TimeBetweenShots=0.15
HitscanRadius=0.0
AAMode=0
[Weapon Profile]
Name=Mute Pistol
Type=Hitscan
Category=SemiAuto
DamagePerShot=5.0
TimeBetweenShots=0.5
HitscanRadius=0.0
AAMode=0
[Bot Rotation Profile]
Name=Duo
ProfileNames=Amber;Teal
ProfileWeights=1.0;1.0
Randomized=true
[Bot Profile]
Name=Amber
CharacterProfile=Amber Ball
UseWeapons=true
NoAiming=true
MinRespawnDelay=0.2
MaxRespawnDelay=0.4
[Bot Profile]
Name=Teal
CharacterProfile=Teal Ball
UseWeapons=true
NoAiming=true
MinRespawnDelay=0.2
MaxRespawnDelay=0.4
[Character Profile]
Name=Amber Ball
MaxHealth=30.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=25.0
MainBBHeight=50.0
MainBBHasHead=false
EnemyBodyColor=X=1.000 Y=0.700 Z=0.000
WeaponProfileNames=Mute Pistol;;;
BlockedSpawnRadius=120.0
MinRespawnDelay=0.2
MaxRespawnDelay=0.4
[Character Profile]
Name=Teal Ball
MaxHealth=40.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=35.0
MainBBHeight=70.0
MainBBHasHead=false
EnemyBodyColor=X=0.000 Y=0.800 Z=0.800
WeaponProfileNames=Mute Pistol;;;
BlockedSpawnRadius=120.0
MinRespawnDelay=0.2
MaxRespawnDelay=0.4
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "400, -300, -150", "rotation": "0, 0, 0", "scale": "0.5, 6, 3"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "350, -150, 20", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "350, 0, 60", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "350, 150, 20", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
