Name=AimAdapt Test - Costly Hop
Author=AimAdapt
Description=Made up for AimAdapt's tests.[nl]One ball hops straight up and pays 30 of its 90 health for each hop, so it goes away on the third hop unless it is shot first.
AimTypeTag=Clicking
Timelimit=20.0
PlayerProfile=Gunner
AddedBots=Hopper.bot
BotTeams=2
ScorePerKill=5.0
ScorePerHit=0.0
ScorePerDamage=0.0
ScoreLossPerMiss=0.0
MapName=Hop Floor.json
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
DamagePerShot=30.0
TimeBetweenShots=0.2
HitscanRadius=0.0
[Bot Profile]
Name=Hopper
CharacterProfile=Hopper Ball
UseWeapons=false
UseAbilityFrequency=1.0
UseAbilityFreqMinTime=1.0
UseAbilityFreqMaxTime=2.0
AbilityUseTimer=0.1
SeeThroughWalls=true
[Character Profile]
Name=Hopper Ball
MaxHealth=90.0
MaxSpeed=0.0
Gravity=1.0
MainBBType=Spheroid
MainBBRadius=30.0
MainBBHeight=60.0
MainBBHasHead=false
EnemyBodyColor=X=0.900 Y=0.700 Z=0.200
MinRespawnDelay=1.0
MaxRespawnDelay=1.0
AbilityProfileNames=Hop.abilmov;;;
[Movement Ability Profile]
Name=Hop
MaxCharges=1.0
ChargeTimer=1.0
ChargesRefundedOnKill=0.0
DelayAfterUse=0.1
FullyAuto=false
AbilityDuration=0.0
LockDirectionForDuration=false
NegateGravityForDuration=false
MainVelocity=0.0
MainVelocityCanGoVertical=false
MainVelocitySetToMovementKeys=false
UpVelocity=400.0
EndVelocityFactor=1.0
Hurtbox=false
AbilityBlocksMovement=false
HealthRestore=-30.0
AIUseInCombat=true
AIUseOutOfCombat=true
AIUseOnGround=true
AIUseInAir=false
AIReuseTimer=0.1
AIMinTargDist=0.0
AIMaxTargDist=9000.0
AIMaxTargFOV=360.0
AIDamageReaction=false
ChargesOnSpawn=1.0
CancelDashOnGreaterThan45Collisions=false
[Map Data]
{"version": "1.0.0", "objects": [
 {"type": "brush", "name": "Default", "mesh": "Cube", "location": "-500, -1500, -250", "rotation": "0, 0, 0", "scale": "30, 30, 1"},
 {"type": "gameObject", "name": "SpawnPoint", "location": "0, 0, 0", "rotation": "0, 0, 0", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 1}, {"name": "Weight", "value": 1.0}]},
 {"type": "gameObject", "name": "SpawnPoint", "location": "1000, 0, -150", "rotation": "0, 0, 180", "scale": "1, 1, 1", "properties": [{"name": "TeamMask", "value": 2}, {"name": "Weight", "value": 1.0}]}
]}
