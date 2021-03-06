name := "parallel-minecraft-downloader"

version := "0.1"

scalaVersion := "2.13.8"

val circeVersion = "0.14.1"

libraryDependencies ++= Seq(
  "io.circe" %% "circe-core",
  "io.circe" %% "circe-generic",
  "io.circe" %% "circe-parser"
).map(_ % circeVersion)
libraryDependencies ++= Seq(
  "org.typelevel" %% "cats-effect" % "3.3.7",
  "com.softwaremill.sttp.client3" %% "core" % "3.5.2",
  "com.softwaremill.sttp.client3" %% "async-http-client-backend-cats" % "3.4.1", // for cats-effect 3.x
  "com.github.cb372" %% "cats-retry" % "3.1.0",
  "org.typelevel" %% "log4cats-slf4j" % "2.2.0",  // Direct Slf4j Support - Recommended
  // logging
  "org.slf4j" % "slf4j-api" % "1.7.35",
  "org.slf4j" % "slf4j-jdk14" % "1.7.35",
  "com.typesafe.scala-logging" % "scala-logging-slf4j_2.10" % "2.1.2",
)

addCompilerPlugin("com.olegpy" %% "better-monadic-for" % "0.3.1")