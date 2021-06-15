package ga.kisaragi.parallelminecraftdownloader

import cats.effect.implicits._
import cats.effect.unsafe.implicits.global
import cats.effect.{IO, Spawn, Sync}
import cats.implicits._
import io.circe.JsonObject
import io.circe.parser._
import sttp.client3._
import sttp.client3.asynchttpclient.cats.AsyncHttpClientCatsBackend

import java.io.File
import java.nio.file.{Files, Paths}
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import scala.concurrent.duration.FiniteDuration
import scala.sys.exit
import scala.util.chaining._

object Main {
  implicit class OptionExtension[T](self: Option[T]) {
    def ??[B >: T](other: => B): B = {
      self.getOrElse(other)
    }
  }

  final val dontDisplayProgress = false
  def main(args: Array[String]): Unit = {
    args.headOption.getOrElse(throw new RuntimeException("provide argument to specify version."))
    val version = args(0)
    // val force = args.length >= 2 && args(1) == "--force"

    val ioBackend = AsyncHttpClientCatsBackend[IO]()
    val counter = new AtomicInteger

    val getManifest = for {
      backend <- ioBackend
      manifest <- for {
        res <- backend.send(basicRequest.get(uri"https://launchermeta.mojang.com/mc/game/version_manifest.json"))
      } yield res.body.getOrElse(throw new RuntimeException("manifest.fetch: failed"))
      parsedManifest = parse(manifest).getOrElse(throw new RuntimeException("manifest.parse: failed"))
    } yield parsedManifest

    val getParsedVersionJson = for {
      backend <- ioBackend
      parsedManifest <- getManifest
      location = parsedManifest
        .hcursor
        .downField("versions")
        .values
        .??(Nil)
        .find(versionEntry => versionEntry.hcursor.downField("id").as[String]
          .getOrElse(throw new RuntimeException("version not found. Check version.")) == version)
        .map(_.hcursor.downField("url").as[String].getOrElse(throw new RuntimeException("1"))) ?? {
        System.err.println("Oops")
        exit(-1)
      }
      _ <- IO { println(s"version_json: found. where=$location") }
      uri = uri"$location"
      res <- backend.send(basicRequest.get(uri))
      body = res.body.getOrElse(throw new RuntimeException("version_json.fetch: failed"))
      parsed = parse(body).getOrElse(throw new RuntimeException("version_json.parse: failed"))
    } yield parsed

    val getParsedAssetIndex = for {
      backend <- ioBackend
      parsedVersionJson <- getParsedVersionJson
      _ <- IO { println("version_json.parse: done") }
      location = parsedVersionJson
        .hcursor
        .downField("assetIndex")
        .downField("url")
        .as[String]
        .getOrElse(throw new IllegalArgumentException())
      uri = uri"$location"
      res <- backend.send(basicRequest.get(uri))
      body = res.body.getOrElse(throw new RuntimeException("asset.fetch: failed"))
      parsed = parse(body).getOrElse(throw new RuntimeException("asset.parse: failed"))
    } yield parsed

    val preprocess = for {
      backend <- ioBackend
      parsedAssetIndex <- getParsedAssetIndex
      assets = parsedAssetIndex
        .hcursor
        .downField("objects")
        .as[JsonObject]
        .getOrElse(throw new IllegalArgumentException("bug"))
        .values
        .map(x => x.hcursor.downField("hash").as[String].getOrElse(throw new IllegalArgumentException("bug or schema changed")))
        .toList

      assetCount = assets.size
      _ <- IO { println(s"asset.count: $assetCount") }
      ios = assets.map(hash => {
        val uri = uri"https://resources.download.minecraft.net/${hash.take(2)}/$hash"
        val dl = for {
          _ <- if (dontDisplayProgress) IO.unit else IO {
            // println(s"download: $uri (hash: $h)")
          }
          bytes <- for {
            res <- backend.send(basicRequest.get(uri).response(asByteArray))
          } yield res.body.getOrElse(throw new IllegalStateException())
          path = Paths.get(System.getProperty("minecraftDirectory"), "assets", "objects", hash.take(2))
            .tap(_.toFile.mkdirs())
            .pipe(m => new File(m.toFile, hash))
            .tap(_.createNewFile())
            .pipe(_.toPath)

          _ <- if (dontDisplayProgress) IO.unit else IO {
            // println(s"write: $hash, path: $path")
          }
          _ <- Spawn[IO].cede
          _ <- IO {
            Files.write(path, bytes)
          }
          _ <- if (dontDisplayProgress) IO.unit else IO {
            print(s"write($hash, ${counter.addAndGet(1)}/$assetCount)")
            val esc = '\u001b'
            print(s"$esc[0K$esc[0G")
          }
        } yield ()

        import retry._
        val retryableIO = retryingOnAllErrors.apply(
          RetryPolicies.constantDelay[IO](new FiniteDuration(100, TimeUnit.MILLISECONDS)),
          (_: Throwable, _) => IO.cede
        )(dl)
        retryableIO
      })
    } yield ios.parSequenceN(128).as(())
    // change that parameter as needed; parUnorderedSequence is worse performance than parSequenceN,
    // because parUnorderedSequence will make tons of request, then SSL handshake became timeout, and recover from that.
    // e.g. mt.parSequenceN
    // - 64: 24.82s
    // - 192: 29.403s
    //parUnorderedSequence: 127.286s
    val whole = for {
      _ <- IO { println("start") }
      _ <- IO { println(s"target version: $version") }
      _ <- IO { println(s"directory: ${System.getProperty("minecraftDirectory")}")}
      start <- IO { System.nanoTime() }
      _ <- preprocess.flatten
      end <- IO { System.nanoTime() }
      _ <- IO {
        println()
        println("end!")
        println(s"time: ${(end - start) / 1000 / 1000}ms")
        exit(0) // needs explicitly.
      }
    } yield ()

    whole.unsafeRunSync()
  }
}
