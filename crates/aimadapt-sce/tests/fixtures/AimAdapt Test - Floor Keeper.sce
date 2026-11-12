Name=AimAdapt Test - Floor Keeper
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]A ball falls toward an invisible floor, and a helper under the floor keeps it up.
AimTypeTag=Tracking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Lifter.bot;Float.bot
BotTeams=1;2
ScorePerKill=0.0
ScorePerHit=1.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Floor Keeper.json
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
Category=FullyAuto
DamagePerShot=1.0
TimeBetweenShots=0.1
HitscanRadius=0.0
AAMode=0
[Bot Profile]
Name=Float
CharacterProfile=Float Ball
UseWeapons=false
[Character Profile]
Name=Float Ball
MaxHealth=5.0
MaxSpeed=0.0
Gravity=1.0
InvincibleBots=true
MainBBType=Spheroid
MainBBRadius=30.0
MainBBHeight=60.0
MainBBHasHead=false
EnemyBodyColor=X=0.900 Y=0.900 Z=0.200
[Bot Profile]
Name=Lifter
CharacterProfile=Lifter Orb
DodgeProfileNames=Rise
DodgeProfileWeights=1.0
UseWeapons=false
NoAiming=false
UseAbilityFrequency=1.0
AbilityUseTimer=0.02
SeeThroughWalls=false
[Dodge Profile]
Name=Rise
ToggleLeftRight=false
ToggleForwardBack=false
InitialForwardMovementState=Forward
[Character Profile]
Name=Lifter Orb
MaxHealth=1.0
MaxSpeed=20000.0
Acceleration=30000.0
DragCoefficient=7.0
IsFlyer=true
MainBBType=Cuboid
MainBBRadius=1.0
MainBBHeight=2.0
MainBBHasHead=true
AbilityProfileNames=Lift.abilmelee;;;
[Melee Ability Profile]
Name=Lift
HurtboxRadius=60.0
HurtboxDamage=0.0
FlatKnockbackHorizontal=300.0
FlatKnockbackVertical=60.0
AbilityDuration=0.01
AIReuseTimer=0.02
ChargeTimer=0.01
DelayAfterUse=0.01
MaxCharges=1.0
AIUseInCombat=true
AIMaxTargFOV=360.0
AIMaxTargDist=100000.0
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "gameObject", "name": "SpawnPoint", "location": "500, 0, -50", "rotation": "0, 90, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "PermittedCharacterProfiles", "value": "Lifter Orb"}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "500, 0, 150", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]},
 {"type": "brush", "name": "Clip", "mesh": "Cube", "location": "-1000, -1000, -50", "rotation": "0, 0, 0", "scale": "30, 20, 0.5"}
]}
