Name=AimAdapt Test - Still Hose
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]Two still balls on a wall, worn down by a weapon that fires while the button is held.
AimTypeTag=Clicking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Pail.bot;Pail.bot
BotTeams=2;2
ScorePerKill=1.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Still Hose.json
MapScale=4.0
InvinciblePlayer=true
BlockHealthbars=true
[Character Profile]
Name=Gunner
MaxHealth=1.0
MaxSpeed=0.0
WeaponProfileNames=Hose;;;
[Weapon Profile]
Name=Hose
Type=Hitscan
Category=FullyAuto
DamagePerShot=1.0
TimeBetweenShots=0.05
HitscanRadius=0.0
[Bot Profile]
Name=Pail
CharacterProfile=Pail Ball
UseWeapons=false
[Character Profile]
Name=Pail Ball
MaxHealth=4.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=40.0
MainBBHeight=80.0
MainBBHasHead=false
EnemyBodyColor=X=0.200 Y=0.500 Z=1.000
MinRespawnDelay=0.3
MaxRespawnDelay=0.3
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "300, -200, -200", "rotation": "0, 0, 0", "scale": "0.5, 4, 4"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "250, -80, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "250, 80, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
