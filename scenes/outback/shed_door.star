# A door on a hinge: used by somebody near it, it swings open, and used again, shut.

def on_begin(self):
    self.vars.open = False
    self.vars.shut = self.yaw
    self.vars.angle = 0.0

def on_interact(self, at):
    offset = at - self.position
    if sqrt(offset.x * offset.x + offset.z * offset.z) > self.reach:
        return
    self.vars.open = not self.vars.open

def on_tick(self, dt):
    target = self.swing if self.vars.open else 0.0
    angle = self.vars.angle
    if angle == target:
        return
    step = self.speed * dt
    if abs(target - angle) <= step:
        angle = target
    elif target > angle:
        angle += step
    else:
        angle -= step
    self.vars.angle = angle
    self.yaw = self.vars.shut + angle
