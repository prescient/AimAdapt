Name=AimAdapt Test - Flinch Kite
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]One flying ball that can't be hurt strafes near the height of its aim line. It never blinks on its own: it blinks along its strafe only when it is hit, after about every tenth shot of the player's automatic weapon, with a short random wait.
AimTypeTag=Tracking
Timelimit=20.0
PlayerProfile=Spotter
AddedBots=Wisp.bot
BotTeams=2
ScorePerKill=0.0
ScorePerHit=1.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Flinch Kite.json
MapScale=1.0
InvinciblePlayer=true
BlockHealthbars=true
[Character Profile]
Name=Spotter
MaxHealth=1.0
MaxSpeed=0.0
MainBBType=Cylindrical
MainBBHeight=170.0
MainBBRadius=35.0
MainBBHasHead=false
CameraOffset=X=0.000 Y=0.000 Z=45.000
SpawnOffsetMin=X=0.000 Y=0.000 Z=0.000
SpawnOffsetMax=X=0.000 Y=0.000 Z=0.000
WeaponProfileNames=Rattler;;;
[Weapon Profile]
Name=Rattler
Type=Hitscan
Category=FullyAuto
DamagePerShot=5.0
TimeBetweenShots=0.05
HitscanRadius=0.0
AAMode=0
[Bot Profile]
Name=Wisp
CharacterProfile=Kite Ball
DodgeProfileNames=Kite Strafe
DodgeProfileWeights=1.0
UseWeapons=false
UseAbilityFrequency=0.0
UseAbilityFreqMinTime=1.0
UseAbilityFreqMaxTime=2.0
AbilityUseTimer=0.8
SeeThroughWalls=false
[Character Profile]
Name=Kite Ball
MaxHealth=3.0
MaxSpeed=350.0
Acceleration=3000.0
Friction=8.0
Gravity=0.0
IsFlyer=true
FlightVelocityUp=300.0
FlightVelocityDown=300.0
FlightAccelUp=2000.0
FlightAccelDown=2000.0
InvincibleBots=true
MainBBType=Spheroid
MainBBRadius=30.0
MainBBHeight=60.0
MainBBHasHead=false
EnemyBodyColor=X=0.300 Y=0.800 Z=0.900
BlockAbilityOnStartDuration=0.2
AbilityGlobalCooldown=0.0
AbilityProfileNames=Flinch Blink.abilmov;;;
[Dodge Profile]
Name=Kite Strafe
ToggleLeftRight=true
MinLRTimeChange=0.5
MaxLRTimeChange=0.8
AlterateJumpCrouchInput=false
[Movement Ability Profile]
Name=Flinch Blink
MaxCharges=6.0
ChargeTimer=0.1
ChargesRefundedOnKill=0.0
DelayAfterUse=0.05
FullyAuto=false
AbilityDuration=0.12
LockDirectionForDuration=true
NegateGravityForDuration=true
MainVelocity=2200.0
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
AIDamageReaction=true
AIDamageReactionThreshold=50.0
AIDamageReactionResetTimer=0.5
AIDamageReactionIgnoreChance=0.0
AIDamageReactionMinDelay=0.1
AIDamageReactionMaxDelay=0.3
AIDamageReactionCooldown=2.0
ChargesOnSpawn=6.0
CancelDashOnGreaterThan45Collisions=false
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1500, -250", "rotation": "0, 0, 0", "scale": "30, 30, 1"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1400, -250", "rotation": "0, 0, 0", "scale": "30, 1, 12"},
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, 1300, -250", "rotation": "0, 0, 0", "scale": "30, 1, 12"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1100, 0, 220", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
