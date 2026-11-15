# Walks where the driver faces, turns with the mouse or the right stick, and uses what is near:
# a door. Physics does the walking: this says which way. While the driver is in the truck this
# hears nothing, and they sit still.

def on_tick(self, dt):
    self.yaw += input("turn") * dt
    speed = self.run_speed if held("run") else self.walk_speed
    ahead = vec3(-sin(self.yaw), 0, -cos(self.yaw))
    right = vec3(cos(self.yaw), 0, -sin(self.yaw))
    jump = self.jump_speed if pressed("jump") else 0
    self.walk = (ahead * input("forward") + right * input("right")) * speed + vec3(0, jump, 0)
    if pressed("interact"):
        send(None, "interact", self.position)
