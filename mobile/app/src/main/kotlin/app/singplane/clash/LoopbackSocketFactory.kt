package app.singplane.clash

import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.Socket
import javax.net.SocketFactory

/**
 * Pre-binds sockets to 127.0.0.1 so Android does not attach the VPN default
 * network. Clash API is loopback-only; a VPN-bound socket cannot reach it.
 */
class LoopbackSocketFactory : SocketFactory() {
    override fun createSocket(): Socket = boundLoopback()

    override fun createSocket(host: String, port: Int): Socket {
        val socket = boundLoopback()
        socket.connect(InetSocketAddress(host, port), CONNECT_MS)
        return socket
    }

    override fun createSocket(host: String, port: Int, localHost: InetAddress, localPort: Int): Socket {
        val socket = Socket()
        socket.bind(InetSocketAddress(loopbackOr(localHost), localPort))
        socket.connect(InetSocketAddress(host, port), CONNECT_MS)
        return socket
    }

    override fun createSocket(host: InetAddress, port: Int): Socket {
        val socket = boundLoopback()
        socket.connect(InetSocketAddress(host, port), CONNECT_MS)
        return socket
    }

    override fun createSocket(address: InetAddress, port: Int, localAddress: InetAddress, localPort: Int): Socket {
        val socket = Socket()
        socket.bind(InetSocketAddress(loopbackOr(localAddress), localPort))
        socket.connect(InetSocketAddress(address, port), CONNECT_MS)
        return socket
    }

    private fun boundLoopback(): Socket {
        val socket = Socket()
        socket.bind(InetSocketAddress(LOOPBACK, 0))
        return socket
    }

    private fun loopbackOr(address: InetAddress): InetAddress =
        if (address.isLoopbackAddress) address else LOOPBACK

    companion object {
        private val LOOPBACK: InetAddress = InetAddress.getByName("127.0.0.1")
        private const val CONNECT_MS = 5_000
    }
}
