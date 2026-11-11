Name=AimAdapt Test - Ledge Kicker
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]A helper kicks two balls sideways, but one hides behind a ledge and is left alone.
AimTypeTag=Clicking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Kicker.bot;Dot.bot;Dot.bot
BotTeams=1;2;2
ScorePerKill=5.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Ledge Kicker.json
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
TimeBetweenShots=0.2
HitscanRadius=0.0
AAMode=0
[Bot Profile]
Name=Dot
CharacterProfile=Dot Ball
UseWeapons=false
[Character Profile]
Name=Dot Ball
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=30.0
MainBBHeight=60.0
MainBBHasHead=false
BlockedSpawnRadius=100.0
EnemyBodyColor=X=0.300 Y=0.900 Z=0.500
[Bot Profile]
Name=Kicker
CharacterProfile=Kicker Orb
NoDodging=true
NoAiming=true
UseWeapons=false
UseAbilityFrequency=1.0
AbilityUseTimer=0.1
SeeThroughWalls=false
[Character Profile]
Name=Kicker Orb
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Spheroid
MainBBRadius=1.0
MainBBHeight=2.0
AbilityProfileNames=Kick.abilmelee;;;
[Melee Ability Profile]
Name=Kick
HurtboxRadius=150.0
HurtboxDamage=0.0
FlatKnockbackHorizontal=400.0
FlatKnockbackVertical=0.0
AbilityDuration=0.01
AIReuseTimer=0.1
ChargeTimer=0.01
DelayAfterUse=0.01
MaxCharges=1.0
AIUseInCombat=true
AIMaxTargFOV=360.0
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "gameObject", "name": "SpawnPoint", "location": "1000, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "PermittedCharacterProfiles", "value": "Kicker Orb"}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1100, -100, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1100, 100, 0", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1500, -50", "rotation": "0, 0, 0", "scale": "40, 30, 0.5"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "1040, 20, 0", "rotation": "0, 0, 0", "scale": "0.2, 0.8, 3"}
]}
