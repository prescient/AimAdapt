Name=AimAdapt Test - High Blink
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]One flying ball starts high above the player, strafes, and blinks along its strafe. Each blink follows its aim line up or down, so it drifts toward the height where that line is level.
AimTypeTag=Tracking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Skimmer.bot
BotTeams=2
ScorePerKill=5.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=High Blink.json
MapScale=1.0
InvinciblePlayer=true
BlockHealthbars=true
[Character Profile]
Name=Gunner
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Cylindrical
MainBBHeight=170.0
MainBBRadius=35.0
MainBBHasHead=false
CameraOffset=X=0.000 Y=0.000 Z=45.000
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
Name=Skimmer
CharacterProfile=Skimmer Ball
DodgeProfileNames=Skim Strafe
DodgeProfileWeights=1.0
UseWeapons=false
UseAbilityFrequency=1.0
UseAbilityFreqMinTime=1.0
UseAbilityFreqMaxTime=2.0
AbilityUseTimer=0.8
SeeThroughWalls=false
[Character Profile]
Name=Skimmer Ball
MaxHealth=3.0
MaxSpeed=400.0
Acceleration=3000.0
Friction=8.0
Gravity=0.0
IsFlyer=true
FlightVelocityUp=300.0
FlightVelocityDown=300.0
FlightAccelUp=2000.0
FlightAccelDown=2000.0
MainBBType=Spheroid
MainBBRadius=30.0
MainBBHeight=60.0
MainBBHasHead=false
EnemyBodyColor=X=1.000 Y=0.600 Z=0.200
BlockAbilityOnStartDuration=0.3
AbilityGlobalCooldown=0.1
AbilityProfileNames=Rise Blink.abilmov;;;
[Dodge Profile]
Name=Skim Strafe
ToggleLeftRight=true
MinLRTimeChange=0.6
MaxLRTimeChange=0.9
AlterateJumpCrouchInput=false
[Movement Ability Profile]
Name=Rise Blink
MaxCharges=3.0
ChargeTimer=0.8
ChargesRefundedOnKill=0.0
DelayAfterUse=0.1
FullyAuto=false
AbilityDuration=0.1
LockDirectionForDuration=true
NegateGravityForDuration=true
MainVelocity=2600.0
MainVelocityCanGoVertical=true
MainVelocitySetToMovementKeys=true
UpVelocity=0.0
EndVelocityFactor=0.0
Hurtbox=false
AbilityBlocksMovement=false
HealthRestore=0.0
AIUseInCombat=true
AIUseOutOfCombat=true
AIUseOnGround=true
AIUseInAir=true
AIReuseTimer=0.0
AIMinTargDist=0.0
AIMaxTargDist=9000.0
AIMaxTargFOV=360.0
AIDamageReaction=false
ChargesOnSpawn=3.0
CancelDashOnGreaterThan45Collisions=false
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1500, -250", "rotation": "0, 0, 0", "scale": "30, 30, 1"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1400, -250", "rotation": "0, 0, 0", "scale": "30, 1, 12"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, 1300, -250", "rotation": "0, 0, 0", "scale": "30, 1, 12"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1100, 0, 500", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
