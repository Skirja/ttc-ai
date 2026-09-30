plugins {
    java
}

dependencies {
    testImplementation("org.junit.jupiter:junit-jupiter:6.0.2")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher:6.0.2")
}

tasks.test {
    useJUnitPlatform()
}

dependencyLocking {
    lockAllConfigurations()
}
