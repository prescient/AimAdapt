Name=AimAdapt Test - Dash Lane
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]One ball strafes down a lane and dashes along its strafe.
AimTypeTag=Tracking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Zipper.bot
BotTeams=2
ScorePerKill=5.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Dash Lane.json
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
Name=Zipper
CharacterProfile=Zipper Ball
DodgeProfileNames=Lane Strafe
DodgeProfileWeights=1.0
UseWeapons=false
UseAbilityFrequency=0.8
UseAbilityFreqMinTime=1.0
UseAbilityFreqMaxTime=2.0
AbilityUseTimer=0.2
SeeThroughWalls=false
[Character Profile]
Name=Zipper Ball
MaxHealth=3.0
MaxSpeed=600.0
Acceleration=3000.0
Friction=8.0
MainBBType=Spheroid
MainBBRadius=40.0
MainBBHeight=80.0
MainBBHasHead=false
EnemyBodyColor=X=0.200 Y=0.800 Z=1.000
BlockAbilityOnStartDuration=0.2
AbilityGlobalCooldown=0.3
AbilityProfileNames=Side Burst.abilmov;;;
[Dodge Profile]
Name=Lane Strafe
ToggleLeftRight=true
MinLRTimeChange=0.5
MaxLRTimeChange=0.8
[Movement Ability Profile]
Name=Side Burst
MaxCharges=2.0
ChargeTimer=1.5
ChargesRefundedOnKill=0.0
DelayAfterUse=0.2
FullyAuto=false
AbilityDuration=0.15
LockDirectionForDuration=true
NegateGravityForDuration=false
MainVelocity=2400.0
MainVelocityCanGoVertical=false
MainVelocitySetToMovementKeys=true
UpVelocity=0.0
EndVelocityFactor=0.4
Hurtbox=false
AbilityBlocksMovement=true
HealthRestore=0.0
AIUseInCombat=true
AIUseOutOfCombat=true
AIUseOnGround=true
AIUseInAir=false
AIReuseTimer=0.3
AIMinTargDist=0.0
AIMaxTargDist=9000.0
AIMaxTargFOV=360.0
AIDamageReaction=false
ChargesOnSpawn=2.0
CancelDashOnGreaterThan45Collisions=false
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1500, -250", "rotation": "0, 0, 0", "scale": "30, 30, 1"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -900, -250", "rotation": "0, 0, 0", "scale": "30, 1, 8"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, 800, -250", "rotation": "0, 0, 0", "scale": "30, 1, 8"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1000, 0, -150", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
